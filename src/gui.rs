// src/gui.rs
use anyhow::Result;
use eframe::egui;
use egui::{Align, Color32, Layout, RichText, ScrollArea, Stroke, Vec2, Ui};
use image::ImageFormat;
use log::{error, info, warn}; // Ensure info and warn are enabled in your logger
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use arboard::{Clipboard, ImageData};

use crate::ai::connector::AiConnector;
use crate::ai::local_model::LocalModel;
use crate::capture::screenshot::ScreenshotManager;
use crate::capture::window_finder::get_window_titles;

const SIDEBAR_WIDTH: f32 = 400.0;
const HANDLE_WIDTH: f32 = 20.0;
const HANDLE_HEIGHT: f32 = 100.0;
const DEFAULT_WINDOW_HEIGHT: f32 = 600.0; 
const CHAT_INPUT_AREA_HEIGHT: f32 = 50.0; 

struct ThreadSafeState {
    processing: bool,
    ai_response: String,
    image_data: Vec<u8>,
    current_image: Option<egui::TextureHandle>,
}

#[derive(Clone)]
struct ChatMessage {
    text: String,
    is_user: bool,
    timestamp: chrono::DateTime<chrono::Local>,
}

pub struct ScreenSnapApp {
    open: bool,
    target_x: f32,
    current_x: f32,
    animation_start_x: f32,
    animation_start_time: Option<Instant>,
    animation_duration: f32,
    was_layout_initialized: bool,
    was_style_initialized: bool,

    screenshot_manager: Arc<Mutex<ScreenshotManager>>,
    state: Arc<Mutex<ThreadSafeState>>,
    model_name: String,
    window_list: Vec<String>,
    selected_window: Option<String>,
    chat_history: Vec<ChatMessage>,
    current_input: String,
}

impl Default for ScreenSnapApp {
    fn default() -> Self {
        let screenshot_manager = ScreenshotManager::new().map_or_else(
            |e| {
                error!("Failed to initialize screenshot manager: {}", e);
                Arc::new(Mutex::new(ScreenshotManager::new().unwrap()))
            },
            |manager| Arc::new(Mutex::new(manager)),
        );
        let window_list = get_window_titles().unwrap_or_else(|e| {
            error!("Failed to get window titles on init: {}", e); Vec::new()
        });
        let state = Arc::new(Mutex::new(ThreadSafeState {
            processing: false, ai_response: String::new(), image_data: Vec::new(), current_image: None,
        }));

        Self {
            open: false, target_x: 0.0, current_x: 0.0, animation_start_x: 0.0,
            animation_start_time: None, animation_duration: 0.3,
            was_layout_initialized: false, 
            was_style_initialized: false, 
            screenshot_manager, state, model_name: "llava:latest".to_string(), window_list,
            selected_window: None, chat_history: Vec::new(), current_input: String::new(),
        }
    }
}

impl eframe::App for ScreenSnapApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.was_style_initialized {
            let mut style = (*ctx.style()).clone();
            style.visuals.window_fill = Color32::TRANSPARENT;
            style.visuals.panel_fill = Color32::TRANSPARENT;
            style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(30, 30, 30);
            style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(45, 45, 45);
            style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(55, 55, 55);
            style.visuals.widgets.active.bg_fill = Color32::from_rgb(65, 65, 65);
            style.visuals.widgets.open.bg_fill = Color32::from_rgb(50, 50, 50);
            style.visuals.widgets.inactive.rounding = egui::Rounding::same(6.0);
            style.visuals.widgets.hovered.rounding = egui::Rounding::same(6.0);
            style.visuals.widgets.active.rounding = egui::Rounding::same(6.0);
            style.visuals.widgets.open.rounding = egui::Rounding::same(6.0);
            style.visuals.selection.bg_fill = Color32::from_rgb(42, 90, 170);
            style.text_styles.insert(
                egui::TextStyle::Body,
                egui::FontId::new(15.0, egui::FontFamily::Proportional)
            );
            style.text_styles.insert(
                egui::TextStyle::Button,
                egui::FontId::new(15.0, egui::FontFamily::Proportional)
            );
            style.text_styles.insert(
                egui::TextStyle::Heading,
                egui::FontId::new(22.0, egui::FontFamily::Proportional)
            );
            ctx.set_style(style);
            self.was_style_initialized = true;
        }

        if !self.was_layout_initialized && ctx.screen_rect().width() > 0.0 {
            let current_app_window_width = ctx.screen_rect().width();
            let initial_x = if self.open { current_app_window_width - SIDEBAR_WIDTH } else { current_app_window_width };
            self.current_x = initial_x;
            self.target_x = initial_x;
            self.animation_start_x = initial_x;
            self.was_layout_initialized = true;
            info!("Layout initialized: app_width={}, initial_x={}", current_app_window_width, initial_x);
        }

        let current_app_window_width_for_sidebar = ctx.screen_rect().width();
        let correct_target_x_for_current_state = if self.open { current_app_window_width_for_sidebar - SIDEBAR_WIDTH } else { current_app_window_width_for_sidebar };

        if self.animation_start_time.is_none() {
            if self.current_x != correct_target_x_for_current_state || self.target_x != correct_target_x_for_current_state {
                // info!("Correcting position: current_x={}, target_x={}, correct_target_x={}", 
                //       self.current_x, self.target_x, correct_target_x_for_current_state);
                self.current_x = correct_target_x_for_current_state;
                self.target_x = correct_target_x_for_current_state;
                self.animation_start_x = self.current_x;
            }
        }

        if let Some(start_time) = self.animation_start_time {
            let elapsed = start_time.elapsed().as_secs_f32();
            let progress = (elapsed / self.animation_duration).min(1.0);
            let ease = 1.0 - (1.0 - progress).powi(3);
            self.current_x = self.animation_start_x + (self.target_x - self.animation_start_x) * ease;
            if progress >= 1.0 {
                info!(
                    "Animation ended. Target_x={}, Attempted current_x={}, Final current_x after set={}",
                    self.target_x, self.current_x, self.target_x // Value it will be set to
                );
                self.current_x = self.target_x;
                self.animation_start_x = self.current_x; 
                self.animation_start_time = None;
            }
            ctx.request_repaint();
        }

        let sidebar_panel_rect = egui::Rect::from_min_size(
            egui::pos2(self.current_x, 0.0),
            egui::vec2(SIDEBAR_WIDTH, ctx.screen_rect().height()),
        );
        if self.current_x < ctx.screen_rect().width() + SIDEBAR_WIDTH { // Draw if any part might be visible or moving
            egui::Area::new("sidebar")
                .fixed_pos(sidebar_panel_rect.min)
                .show(ctx, |ui| {
                    // info!("Drawing sidebar Area at x: {}, width: {}", sidebar_panel_rect.min.x, SIDEBAR_WIDTH);
                    egui::Frame::dark_canvas(ui.style())
                        .fill(Color32::from_rgb(25, 25, 25))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(70, 70, 70)))
                        .shadow(egui::epaint::Shadow {
                            extrusion: 8.0, 
                            color: Color32::from_black_alpha(80),
                        })
                        .show(ui, |frame_ui| { 
                            frame_ui.set_max_width(SIDEBAR_WIDTH); // This ensures the Ui inside the frame has this max_width
                            frame_ui.set_min_width(SIDEBAR_WIDTH); // Explicitly set min_width too
                            frame_ui.set_min_height(ctx.screen_rect().height());
                            // info!("Frame UI for sidebar content: available_width={}", frame_ui.available_width());
                            self.draw_sidebar_contents(frame_ui, ctx);
                        });
                });
        }

        let handle_x_pos = self.current_x - HANDLE_WIDTH;
        let handle_center_y = (ctx.screen_rect().height() - HANDLE_HEIGHT) / 2.0f32;
        let time = ctx.input(|i| i.time);
        let bobbing_offset_f64 = (time * 1.5).sin() * 3.0;
        let bobbing_offset_f32 = bobbing_offset_f64 as f32;
        let handle_rect = egui::Rect::from_min_size(
            egui::pos2(handle_x_pos, handle_center_y + bobbing_offset_f32),
            egui::vec2(HANDLE_WIDTH, HANDLE_HEIGHT),
        );
        egui::Area::new("handle")
            .fixed_pos(handle_rect.min)
            .show(ctx, |ui| {
                egui::Frame::dark_canvas(ui.style())
                    .fill(Color32::from_rgb(42, 90, 170))
                    .rounding(egui::Rounding::same(10.0))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(120, 150, 200)))
                    .shadow(egui::epaint::Shadow {
                        extrusion: 5.0,
                        color: Color32::from_black_alpha(100),
                    })
                    .show(ui, |ui| {
                        ui.set_max_width(HANDLE_WIDTH);
                        ui.set_min_height(HANDLE_HEIGHT);
                        ui.with_layout(Layout::centered_and_justified(egui::Direction::TopDown), |ui| {
                            let icon = if self.open { "▶" } else { "◀" };
                            if ui.add(egui::Button::new(RichText::new(icon).size(16.0).color(Color32::WHITE))
                                .fill(Color32::TRANSPARENT)
                                .frame(false)
                            ).clicked() {
                                self.open = !self.open;
                                let app_w = ctx.screen_rect().width();
                                let new_target_x = if self.open { // If NOW open
                                    app_w - SIDEBAR_WIDTH
                                } else { // If NOW closed
                                    app_w
                                };
                                info!(
                                    "Handle clicked. self.open={}, app_width={}, SIDEBAR_WIDTH={}, HANDLE_WIDTH={}, new_target_x={}. current_x was {}",
                                    self.open, app_w, SIDEBAR_WIDTH, HANDLE_WIDTH, new_target_x, self.current_x
                                );
                                self.target_x = new_target_x;
                                self.animation_start_x = self.current_x;
                                self.animation_start_time = Some(Instant::now());
                            }
                        });
                    });
            });
    }
}

impl ScreenSnapApp {
    fn draw_sidebar_contents(&mut self, frame_ui: &mut Ui, ctx: &egui::Context) {
        let app_window_width_for_sidebar_logic = ctx.screen_rect().width();
        
        // This ensures that all content added to frame_ui respects the sidebar's width
        // frame_ui.set_max_width(SIDEBAR_WIDTH); // Already set by the caller Frame

        let top_section_response = frame_ui.vertical(|ui| { // Capture response of the top section
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.heading(RichText::new("ScreenSnap AI").size(22.0));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(RichText::new("✕").size(16.0)).clicked() {
                        self.open = false;
                        self.target_x = app_window_width_for_sidebar_logic; // Should use ctx.screen_rect().width()
                        self.animation_start_x = self.current_x;
                        self.animation_start_time = Some(Instant::now());
                    }
                });
            });
            ui.separator();
            ui.add_space(8.0);
            
            ui.horizontal(|ui| {
                let button_size = egui::vec2(ui.available_width() * 0.5 - 4.0, 36.0);
                if ui.add_sized(button_size, egui::Button::new(
                    RichText::new("📷 Capture Screen").size(14.0))
                    .fill(Color32::from_rgb(45, 45, 45))
                    .rounding(8.0)
                ).clicked() {
                    self.capture_full_screen();
                }
                ui.add_space(8.0);
                if ui.add_sized(button_size, egui::Button::new(
                    RichText::new("🪟 Capture Window").size(14.0))
                    .fill(Color32::from_rgb(45, 45, 45))
                    .rounding(8.0)
                ).clicked() {
                    match get_window_titles() {
                        Ok(list) => self.window_list = list,
                        Err(e) => error!("Failed to get window list: {}", e),
                    }
                    if !self.window_list.is_empty() && self.selected_window.is_none() {
                        self.selected_window = Some(self.window_list[0].clone());
                    }
                }
            });

            let mut wants_to_capture_selected_window = false;
            let current_selection_display = self.selected_window.clone();
            if let Some(selected_name_for_combo) = &current_selection_display {
                ui.add_space(4.0);
                egui::Frame::none()
                    .fill(Color32::from_rgb(35, 35, 35))
                    .rounding(8.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Window:").size(14.0));
                            let mut new_selection_from_combo_this_frame: Option<String> = None;
                            egui::ComboBox::from_id_source("window_selector")
                                .selected_text(selected_name_for_combo.as_str())
                                .width(ui.available_width() - 90.0)
                                .show_ui(ui, |ui| {
                                    for window_title in &self.window_list {
                                        let is_selected = self.selected_window.as_ref() == Some(window_title);
                                        let truncated = if window_title.len() > 40 {
                                            format!("{}...", &window_title[..40])
                                        } else {
                                            window_title.clone()
                                        };
                                        if ui.selectable_label(is_selected, truncated).clicked() {
                                            new_selection_from_combo_this_frame = Some(window_title.clone());
                                        }
                                    }
                                });
                            if let Some(new_sel) = new_selection_from_combo_this_frame {
                                self.selected_window = Some(new_sel);
                            }
                            if ui.add_sized([80.0, 24.0], egui::Button::new("Capture")
                                .fill(Color32::from_rgb(42, 90, 170))
                                .rounding(4.0)
                            ).clicked() {
                                if self.selected_window.is_some() {
                                    wants_to_capture_selected_window = true;
                                }
                            }
                        });
                    });
            }
            if wants_to_capture_selected_window {
                self.capture_selected_window();
            }

            ui.add_space(8.0);
            let mut should_analyze = false;
            egui::Frame::none()
                .fill(Color32::from_rgb(35, 35, 35))
                .rounding(8.0)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Model:").size(14.0));
                        let current_model_name_for_combo = self.model_name.clone();
                        egui::ComboBox::from_id_source("model_selector")
                            .selected_text(&current_model_name_for_combo)
                            .width(ui.available_width() - 100.0)
                            .show_ui(ui, |ui| {
                                for model_choice in &["llava:latest", "llava:13b", "llava:7b"] {
                                    if ui.selectable_label(self.model_name == *model_choice, *model_choice).clicked() {
                                        self.model_name = model_choice.to_string();
                                    }
                                }
                            });
                        let (is_processing, has_image_data) = {
                            let state_guard = self.state.lock().unwrap();
                            (state_guard.processing, !state_guard.image_data.is_empty())
                        };
                        if is_processing {
                            ui.spinner();
                        } else if has_image_data {
                            if ui.add_sized([90.0, 28.0], egui::Button::new(
                                RichText::new("🤖 Analyze").size(14.0))
                                .fill(Color32::from_rgb(42, 90, 170))
                                .rounding(4.0)
                            ).clicked() {
                                should_analyze = true;
                            }
                        }
                    });
                });
            if should_analyze {
                self.analyze_image();
            }
        }).response; // Get the response of the vertical layout for its rect


        let image_to_load_opt: Option<image::DynamicImage> = {
            let state_guard = self.state.lock().unwrap();
            let should_load_texture = state_guard.current_image.is_none() && !state_guard.image_data.is_empty();
            drop(state_guard);
            if should_load_texture {
                if let Ok(manager) = self.screenshot_manager.lock() {
                    manager.get_current_image().cloned()
                } else { None }
            } else { None }
        };
        if let Some(image_data_cloned) = image_to_load_opt {
            let mut state_guard = self.state.lock().unwrap();
            let size = [image_data_cloned.width() as usize, image_data_cloned.height() as usize];
            let egui_image = egui::ColorImage::from_rgba_unmultiplied(
                size,
                image_data_cloned.to_rgba8().as_flat_samples().as_slice(),
            );
            state_guard.current_image = Some(ctx.load_texture(
                "screenshot_texture",
                egui_image,
                egui::TextureOptions::LINEAR,
            ));
        }

        let (texture_handle_clone, ai_response_cloned, processing_cloned, is_image_texture_available) = {
            let state_guard = self.state.lock().unwrap();
            (
                state_guard.current_image.clone(),
                state_guard.ai_response.clone(),
                state_guard.processing,
                state_guard.current_image.is_some()
            )
        };
        
        let full_sidebar_rect = frame_ui.max_rect(); 
        let top_section_bottom = top_section_response.rect.bottom(); // Use the bottom of the actually rendered top section

        let scroll_area_top = top_section_bottom;
        let scroll_area_bottom = full_sidebar_rect.bottom() - CHAT_INPUT_AREA_HEIGHT;
        
        let scroll_area_rect = egui::Rect::from_min_max(
            egui::pos2(full_sidebar_rect.left(), scroll_area_top),
            egui::pos2(full_sidebar_rect.right(), scroll_area_bottom)
        );

        if scroll_area_rect.height() > 0.0 { 
            frame_ui.allocate_ui_at_rect(scroll_area_rect, |scroll_ui| {
                if is_image_texture_available || !ai_response_cloned.is_empty() || !self.chat_history.is_empty() {
                    // Only add separator if there's content to separate from top section
                    if top_section_response.rect.height() > 0.0 { // Check if top section actually drew anything
                       // scroll_ui.separator(); // Separator can be part of scroll content if desired
                    }
                }
                ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true)
                    .show(scroll_ui, |inner_scroll_ui| {
                        if is_image_texture_available || !ai_response_cloned.is_empty() || !self.chat_history.is_empty() {
                             inner_scroll_ui.separator(); // Separator at the top of scroll content
                        }
                        if let Some(texture) = &texture_handle_clone {
                            inner_scroll_ui.add_space(5.0);
                            inner_scroll_ui.heading(RichText::new("Screenshot").size(18.0));
                            inner_scroll_ui.add_space(5.0);
                            let available_width = inner_scroll_ui.available_width().min(SIDEBAR_WIDTH - 20.0);
                            let aspect_ratio = texture.size_vec2().x / texture.size_vec2().y;
                            let image_height = if aspect_ratio > 0.0 { available_width / aspect_ratio } else { available_width };
                            let image_size = Vec2::new(available_width, image_height);
                            inner_scroll_ui.image((texture.id(), image_size));
                            inner_scroll_ui.horizontal(|h_ui| {
                                if h_ui.add_sized([h_ui.available_width() * 0.5 - 4.0, 32.0], 
                                    egui::Button::new(RichText::new("💾 Save Image").size(14.0))
                                    .fill(Color32::from_rgb(45, 45, 45)).rounding(6.0)).clicked() {
                                    if let Some(path) = rfd::FileDialog::new().add_filter("PNG", &["png"]).add_filter("JPEG", &["jpg", "jpeg"]).set_file_name("screenshot.png").save_file() {
                                        self.save_image(path);
                                    }
                                }
                                h_ui.add_space(8.0);
                                if h_ui.add_sized([h_ui.available_width(), 32.0], egui::Button::new(RichText::new("📋 Copy").size(14.0))
                                    .fill(Color32::from_rgb(45, 45, 45)).rounding(6.0)).clicked() {
                                    self.copy_image_to_clipboard();
                                }
                            });
                            inner_scroll_ui.add_space(8.0);
                        }

                        if !self.chat_history.is_empty() {
                            inner_scroll_ui.add_space(8.0);
                            inner_scroll_ui.heading(RichText::new("Chat History").size(18.0));
                            inner_scroll_ui.add_space(8.0);
                            for message in &self.chat_history {
                                self.draw_chat_message(inner_scroll_ui, message);
                            }
                        }

                        if !ai_response_cloned.is_empty() {
                            let is_new_ai_message = self.chat_history.last().map_or(true, |m| m.text != ai_response_cloned || m.is_user);
                            if is_new_ai_message && self.chat_history.is_empty() { inner_scroll_ui.add_space(8.0); inner_scroll_ui.heading(RichText::new("AI Response").size(18.0)); inner_scroll_ui.add_space(5.0); }
                            else if is_new_ai_message { inner_scroll_ui.add_space(5.0); }
                            let ai_message_for_display = ChatMessage { text: ai_response_cloned.clone(), is_user: false, timestamp: chrono::Local::now() };
                            self.draw_chat_message(inner_scroll_ui, &ai_message_for_display);
                            if !processing_cloned && is_new_ai_message {
                                self.chat_history.push(ai_message_for_display.clone());
                                let mut state_guard = self.state.lock().unwrap();
                                if state_guard.ai_response == ai_response_cloned { state_guard.ai_response.clear(); }
                            }
                        }
                    });
            });
        }


        let input_area_rect = egui::Rect::from_min_max(
            egui::pos2(full_sidebar_rect.left(), full_sidebar_rect.bottom() - CHAT_INPUT_AREA_HEIGHT),
            egui::pos2(full_sidebar_rect.right(), full_sidebar_rect.bottom())
        );
        frame_ui.allocate_ui_at_rect(input_area_rect, |input_ui| {
            self.draw_modern_chat_input(input_ui);
        });
    }


    fn draw_chat_message(&self, ui: &mut Ui, message: &ChatMessage) {
        let (bubble_color, text_color, name_text, name_color) = if message.is_user {
            (Color32::from_rgb(42, 90, 170), Color32::WHITE, "You", Color32::from_rgb(220, 220, 220))
        } else {
            (Color32::from_rgb(50, 50, 50), Color32::WHITE, "AI", Color32::from_rgb(180, 180, 180))
        };
        let layout_alignment = if message.is_user { Align::RIGHT } else { Align::LEFT };
        ui.with_layout(Layout::top_down(layout_alignment), |ui| {
            let time_str = message.timestamp.format("%H:%M").to_string();
            ui.horizontal(|ui| {
                if !message.is_user {
                    ui.label(RichText::new(name_text).color(name_color).small());
                    ui.label(RichText::new(time_str).color(Color32::from_rgb(130, 130, 130)).small());
                } else {
                    ui.label(RichText::new(time_str).color(Color32::from_rgb(130, 130, 130)).small());
                    ui.label(RichText::new(name_text).color(name_color).small());
                }
            });
            egui::Frame::none()
                .fill(bubble_color)
                .rounding(if message.is_user {
                    egui::Rounding { nw: 15.0, ne: 4.0, sw: 15.0, se: 15.0 }
                } else {
                    egui::Rounding { nw: 4.0, ne: 15.0, sw: 15.0, se: 15.0 }
                })
                .stroke(Stroke::new(1.0, bubble_color.linear_multiply(0.9)))
                .shadow(egui::epaint::Shadow {
                    extrusion: 4.0, 
                    color: Color32::from_black_alpha(40),
                })
                .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                .show(ui, |ui| {
                    ui.set_max_width(SIDEBAR_WIDTH * 0.8); 
                    ui.label(RichText::new(&message.text).color(text_color)); 
                });
            ui.add_space(6.0);
        });
    }

    fn draw_modern_chat_input(&mut self, ui: &mut Ui) -> bool {
        let mut message_sent = false;
        egui::Frame::none() 
            .fill(Color32::from_rgb(35, 35, 35))
            .rounding(8.0)
            .stroke(Stroke::new(1.0, Color32::from_rgb(60, 60, 60)))
            .inner_margin(egui::Margin::symmetric(8.0, 4.0))
            .show(ui, |ui| {
                ui.centered_and_justified(|ui| { 
                    ui.horizontal(|ui| {
                        let text_edit = egui::TextEdit::singleline(&mut self.current_input)
                            .hint_text("Type a message or /help...")
                            .desired_width(ui.available_width() - 44.0) 
                            .margin(egui::vec2(8.0, 6.0))
                            .font(egui::TextStyle::Body);
                        let response = ui.add(text_edit);
                        ui.add_space(4.0);
                        let send_button = ui.add_sized(
                            [36.0, 36.0], 
                            egui::Button::new(RichText::new("⮞").size(16.0))
                                .fill(Color32::from_rgb(42, 90, 170))
                                .rounding(18.0)
                        );
                        let should_send = send_button.clicked() || 
                            (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.current_input.is_empty());
                        if should_send {
                            let user_message_text = self.current_input.trim().to_string();
                            if !user_message_text.is_empty() {
                                self.current_input.clear();
                                let user_message = ChatMessage {
                                    text: user_message_text.clone(),
                                    is_user: true,
                                    timestamp: chrono::Local::now(),
                                };
                                info!("Adding user message to chat history: '{}'", &user_message.text);
                                self.chat_history.push(user_message);
                                self.handle_user_input(user_message_text); 
                                message_sent = true;
                                response.request_focus();
                            } else {
                                self.current_input.clear();
                            }
                        }
                    });
                });
            });
        message_sent
    }

    fn capture_full_screen(&mut self) {
        let screenshot_manager_clone = Arc::clone(&self.screenshot_manager);
        let state_clone = Arc::clone(&self.state);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            if let Ok(mut manager) = screenshot_manager_clone.lock() {
                if let Err(e) = manager.capture_screen() {
                    error!("Failed to capture screen: {}", e);
                } else {
                    if let Ok(image_data_bytes) = manager.get_current_image_data() {
                        let mut state = state_clone.lock().unwrap();
                        state.image_data = image_data_bytes;
                        state.current_image = None; 
                        info!("Full screen captured, image data updated.");
                    }
                }
            }
        });
    }

    fn capture_selected_window(&mut self) {
        if let Some(window_title_owned) = self.selected_window.clone() {
            let screenshot_manager_clone = Arc::clone(&self.screenshot_manager);
            let state_clone = Arc::clone(&self.state);
            thread::spawn(move || {
                if let Ok(mut manager) = screenshot_manager_clone.lock() {
                    if let Err(e) = manager.capture_window(&window_title_owned) {
                        error!("Failed to capture window '{}': {}", window_title_owned, e);
                        if manager.capture_screen().is_ok() { 
                            if let Ok(image_data_bytes) = manager.get_current_image_data() {
                                let mut state = state_clone.lock().unwrap();
                                state.image_data = image_data_bytes;
                                state.current_image = None; 
                                info!("Window capture failed, fell back to full screen. Image data updated.");
                            }
                        } else {
                             error!("Fallback to full screen capture also failed");
                        }
                    } else {
                        if let Ok(image_data_bytes) = manager.get_current_image_data() {
                            let mut state = state_clone.lock().unwrap();
                            state.image_data = image_data_bytes;
                            state.current_image = None; 
                            info!("Window '{}' captured, image data updated.", window_title_owned);
                        }
                    }
                }
            });
        }
    }

    fn analyze_image(&mut self) {
        let image_data_bytes = {
            let mut state_guard = self.state.lock().unwrap(); 
            if state_guard.image_data.is_empty() {
                info!("No image data to analyze.");
                state_guard.ai_response = "Please capture an image first.".to_string();
                return;
            }
            state_guard.image_data.clone()
        };
        let model_name = self.model_name.clone();
        let state_clone = Arc::clone(&self.state);
        {
            let mut state_guard = self.state.lock().unwrap();
            state_guard.processing = true;
            state_guard.ai_response = "Processing image...".to_string(); 
        }
        info!("Starting AI analysis for image.");
        thread::spawn(move || {
            std::env::set_var("OLLAMA_HOST", &get_ollama_url(None));
            match LocalModel::new(&model_name) {
                Ok(mut ai_model) => {
                    match ai_model.process_image(&image_data_bytes) {
                        Ok(response) => {
                            let mut state_guard = state_clone.lock().unwrap();
                            state_guard.ai_response = response;
                            info!("AI analysis complete.");
                        }
                        Err(e) => {
                            let mut state_guard = state_clone.lock().unwrap();
                            state_guard.ai_response = format!("AI processing failed: {}", e);
                            if e.to_string().contains("not found") {
                                state_guard.ai_response.push_str(&format!("\n\nTo fix: ollama pull {}", model_name));
                            } else if e.to_string().contains("not available") || e.to_string().contains("connection refused") {
                                state_guard.ai_response.push_str("\n\nEnsure Ollama is running: ollama serve");
                            }
                            error!("AI processing error: {}", e);
                        }
                    }
                }
                Err(e) => {
                    let mut state_guard = state_clone.lock().unwrap();
                    state_guard.ai_response = format!("Failed to init Ollama model: {}\n\n", e);
                    state_guard.ai_response.push_str("Is Ollama running? Is model pulled?");
                    error!("Failed to init Ollama model: {}", e);
                }
            }
            let mut state_guard = state_clone.lock().unwrap();
            state_guard.processing = false;
        });
    }

    fn handle_user_input(&mut self, input: String) {
        info!("Handling user input: '{}'", input);
        if input.starts_with('/') {
            let parts: Vec<&str> = input.splitn(2, ' ').collect();
            let command = parts[0].to_lowercase();
            let mut response_text = String::new(); 

            match command.as_str() {
                "/capture" => self.capture_full_screen(),
                "/window" => {
                    match get_window_titles() {
                        Ok(list) => self.window_list = list,
                        Err(e) => error!("Failed to get window list: {}", e),
                    }
                    if parts.len() > 1 {
                        let window_name = parts[1].trim();
                        let matched_window = self.window_list.iter()
                            .find(|w| w.to_lowercase().contains(&window_name.to_lowercase())) 
                            .cloned();
                        if let Some(window) = matched_window {
                            self.selected_window = Some(window);
                            self.capture_selected_window();
                        } else {
                            self.selected_window = Some(window_name.to_string()); 
                            self.capture_selected_window();
                        }
                    } else {
                        response_text = "Please specify a window name or part of it after /window (e.g., /window firefox)".to_string();
                    }
                },
                "/model" => {
                    if parts.len() > 1 {
                        let model_name_input = parts[1].trim();
                        self.model_name = model_name_input.to_string();
                        response_text = format!("Model set to: {}", self.model_name);
                    } else {
                        response_text = format!("Current model: {}. Usage: /model <model_name>", self.model_name);
                    }
                },
                "/analyze" => {
                    let mut state_guard_check = self.state.lock().unwrap(); 
                    if state_guard_check.image_data.is_empty() {
                        response_text = "Please capture an image first using /capture or /window.".to_string();
                    } else {
                        drop(state_guard_check); 
                        self.analyze_image();
                    }
                },
                "/clear" => {
                    self.chat_history.clear();
                    let mut state_guard = self.state.lock().unwrap();
                    state_guard.current_image = None; 
                    state_guard.image_data.clear();
                    state_guard.ai_response.clear();
                    info!("Chat history and current image cleared.");
                    response_text = "Chat history and image cleared.".to_string();
                },
                "/help" => {
                    response_text = "Available commands:\n\
                        /capture - Capture full screen\n\
                        /window [name] - Capture a specific window (or part of name)\n\
                        /model [name] - Change AI model (e.g., /model llava:latest)\n\
                        /analyze - Analyze current image with default prompt\n\
                        /clear - Clear chat history and current image\n\
                        /help - Show this help message".to_string();
                },
                _ => {
                    response_text = format!("Unknown command: {}. Type /help for available commands.", command);
                }
            }
            if !response_text.is_empty() {
                let mut state_guard = self.state.lock().unwrap();
                state_guard.ai_response = response_text; 
            }
        } else { 
            let mut state_guard_check = self.state.lock().unwrap(); 
            if state_guard_check.image_data.is_empty() {
                state_guard_check.ai_response = "Please capture an image first before sending a prompt.".to_string();
            } else {
                drop(state_guard_check); 
                self.analyze_with_prompt(input);
            }
        }
    }

    fn analyze_with_prompt(&mut self, prompt: String) {
        info!("Analyzing with prompt: '{}'", prompt);
        let image_data_bytes = {
            let mut state_guard = self.state.lock().unwrap(); 
            if state_guard.image_data.is_empty() {
                state_guard.ai_response = "Please capture an image for prompt analysis.".to_string();
                return;
            }
            state_guard.image_data.clone()
        };
        let model_name = self.model_name.clone();
        let state_clone = Arc::clone(&self.state);
        let prompt_clone = prompt.clone();
        {
            let mut state_guard = self.state.lock().unwrap();
            state_guard.processing = true;
            state_guard.ai_response = "Processing with your prompt...".to_string();
        }
        thread::spawn(move || {
            std::env::set_var("OLLAMA_HOST", &get_ollama_url(None));
            match LocalModel::new(&model_name) {
                Ok(mut ai_model) => {
                    ai_model.set_prompt(&prompt_clone); 
                    match ai_model.process_image(&image_data_bytes) {
                        Ok(response) => {
                            let mut state_guard = state_clone.lock().unwrap();
                            state_guard.ai_response = response;
                            info!("AI analysis with prompt complete.");
                        }
                        Err(e) => {
                            let mut state_guard = state_clone.lock().unwrap();
                            state_guard.ai_response = format!("AI processing failed: {}", e);
                            if e.to_string().contains("not found") {
                                state_guard.ai_response.push_str(&format!("\n\nTo fix: ollama pull {}", model_name));
                            } else if e.to_string().contains("not available") || e.to_string().contains("connection refused") {
                                state_guard.ai_response.push_str("\n\nEnsure Ollama is running: ollama serve");
                            }
                             error!("AI processing with prompt error: {}", e);
                        }
                    }
                }
                Err(e) => {
                    let mut state_guard = state_clone.lock().unwrap();
                    state_guard.ai_response = format!("Failed to init Ollama model: {}\n\n", e);
                    state_guard.ai_response.push_str("Is Ollama running? Is model pulled?");
                    error!("Failed to init Ollama model for prompt analysis: {}", e);
                }
            }
            let mut state_guard = state_clone.lock().unwrap();
            state_guard.processing = false;
        });
    }

    fn save_image(&self, path: PathBuf) {
        if let Ok(manager) = self.screenshot_manager.lock() {
            if let Some(image) = manager.get_current_image() {
                if let Err(e) = image.save_with_format(&path, ImageFormat::Png) {
                    error!("Failed to save image: {}", e);
                } else {
                    info!("Image saved to: {}", path.display());
                }
            }
        }
    }

    fn copy_image_to_clipboard(&self) {
        #[cfg(feature = "clipboard")]
        {
            if let Ok(manager) = self.screenshot_manager.lock() {
                if let Some(image) = manager.get_current_image() {
                    let width = image.width() as usize;
                    let height = image.height() as usize;
                    let rgba8 = image.to_rgba8();
                    match Clipboard::new() {
                        Ok(mut clipboard) => {
                            let img_data = ImageData {
                                width,
                                height,
                                bytes: rgba8.as_raw().into(),
                            };
                            if let Err(e) = clipboard.set_image(img_data) {
                                error!("Failed to copy image to clipboard: {}", e);
                            } else {
                                info!("Image copied to clipboard");
                            }
                        }
                        Err(e) => {
                            error!("Failed to access clipboard: {}", e);
                        }
                    }
                }
            }
        }
        #[cfg(not(feature = "clipboard"))]
        {
            let mut state_guard = self.state.lock().unwrap();
            state_guard.ai_response = "Clipboard feature not enabled in this build.".to_string();
            error!("Clipboard feature not enabled. Enable the 'clipboard' feature in Cargo.toml");
        }
    }
}

fn get_ollama_url(url_arg: Option<String>) -> String {
    url_arg.unwrap_or_else(|| {
        std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string())
    })
}

pub fn run_gui() -> Result<()> {
    info!("ScreenSnap GUI starting up...");

    let mut mon_abs_x = 0.0f32;
    let mut mon_abs_y = 0.0f32;
    let mut mon_width = 1920.0f32; 
    let mut mon_height = 1080.0f32; 

    match screenshots::Screen::all() {
        Ok(screens) => {
            if screens.is_empty() {
                error!("run_gui: No screens found by screenshots crate. Using default values for positioning.");
            } else {
                let primary_screen_opt = screens.iter().find(|s| s.display_info.is_primary);
                let screen_to_use = primary_screen_opt.unwrap_or_else(|| {
                    warn!("run_gui: Could not identify primary screen. Using the first screen found.");
                    &screens[0]
                });
                mon_abs_x = screen_to_use.display_info.x as f32;
                mon_abs_y = screen_to_use.display_info.y as f32;
                mon_width = screen_to_use.display_info.width as f32;
                mon_height = screen_to_use.display_info.height as f32;
                info!(
                    "run_gui: Using screen for positioning: AbsX={}, AbsY={}, Width={}, Height={}",
                    mon_abs_x, mon_abs_y, mon_width, mon_height
                );
            }
        }
        Err(e) => {
            error!("run_gui: Failed to get screen info via screenshots crate: {}. Using default values.", e);
        }
    }

    let app_window_width = SIDEBAR_WIDTH + HANDLE_WIDTH;
    let app_window_height = DEFAULT_WINDOW_HEIGHT;
    let desired_x = mon_abs_x + mon_width - app_window_width;
    let taskbar_buffer = 40.0; 
    let desired_y = mon_abs_y + mon_height - app_window_height - taskbar_buffer;
    
    info!("run_gui: Calculated initial window position: x={}, y={}", desired_x, desired_y);

    let native_options = eframe::NativeOptions {
        initial_window_pos: Some(egui::pos2(desired_x.max(0.0), desired_y.max(0.0))),
        initial_window_size: Some(egui::vec2(app_window_width, app_window_height)),
        transparent: true,
        decorated: false,
        always_on_top: true,
        fullscreen: false,
        ..eframe::NativeOptions::default()
    };

    eframe::run_native(
        "ScreenSnap",
        native_options,
        Box::new(|_cc| {
            Box::new(ScreenSnapApp::default())
        }),
    )
    .map_err(|e| anyhow::anyhow!("Failed to start GUI: {}", e))?;

    Ok(())
}