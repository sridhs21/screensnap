// src/capture/screenshot.rs
use anyhow::{Result, anyhow};
use image::DynamicImage;
use screenshots::Screen;
use std::io::Cursor;
use log::info;
use super::window_finder;

pub struct ScreenshotManager {
    current_image: Option<DynamicImage>,
}

impl ScreenshotManager {
    pub fn new() -> Result<Self> {
        Ok(Self {
            current_image: None,
        })
    }

    /// Capture the entire primary screen
    pub fn capture_screen(&mut self) -> Result<()> {
        info!("Capturing primary screen");
        // Get all screens
        let screens = Screen::all()?;
        if screens.is_empty() {
            return Err(anyhow!("No screens found"));
        }
        
        // Use the primary screen (first one)
        let screen = &screens[0];
        let image = screen.capture()?;
        
        // Convert to DynamicImage
        let width = image.width() as u32;
        let height = image.height() as u32;
        
        // Get raw data - the screenshots crate returns BGRA format
        let buffer = image.as_raw().to_vec();
        
        // Convert BGRA to RGBA
        let mut rgba_buffer = Vec::with_capacity(buffer.len());
        for chunk in buffer.chunks(4) {
            if chunk.len() == 4 {
                rgba_buffer.push(chunk[2]); // R
                rgba_buffer.push(chunk[1]); // G
                rgba_buffer.push(chunk[0]); // B
                rgba_buffer.push(chunk[3]); // A
            }
        }
        
        let rgba = image::RgbaImage::from_raw(width, height, rgba_buffer)
            .ok_or_else(|| anyhow!("Failed to create image from raw data"))?;
        
        let dynamic_image = DynamicImage::ImageRgba8(rgba);
        self.current_image = Some(dynamic_image);
        
        info!("Screen captured: {}x{}", width, height);
        Ok(())
    }

    /// Capture a specific window by its title
    pub fn capture_window(&mut self, window_title: &str) -> Result<()> {
        info!("Capturing window: {}", window_title);
        // Get window bounds
        let window_bounds = window_finder::get_window_bounds(window_title)?;
        
        // Capture the region
        let screens = Screen::all()?;
        if screens.is_empty() {
            return Err(anyhow!("No screens found"));
        }
        
        // Find appropriate screen
        let screen = screens.iter().find(|s| {
            let bounds = s.display_info;
            window_bounds.x >= bounds.x as i32 &&
            window_bounds.y >= bounds.y as i32 &&
            (window_bounds.x + window_bounds.width as i32) <= (bounds.x as i32 + bounds.width as i32) &&
            (window_bounds.y + window_bounds.height as i32) <= (bounds.y as i32 + bounds.height as i32)
        }).unwrap_or(&screens[0]);
        
        // Calculate the capture region relative to the screen
        let capture_x = window_bounds.x - screen.display_info.x as i32;
        let capture_y = window_bounds.y - screen.display_info.y as i32;
        
        let image = screen.capture_area(
            capture_x.max(0) as i32,
            capture_y.max(0) as i32,
            window_bounds.width as u32,
            window_bounds.height as u32
        )?;
        
        // Convert to DynamicImage
        let width = image.width() as u32;
        let height = image.height() as u32;
        
        // Get raw data - the screenshots crate returns BGRA format
        let buffer = image.as_raw().to_vec();
        
        // Convert BGRA to RGBA
        let mut rgba_buffer = Vec::with_capacity(buffer.len());
        for chunk in buffer.chunks(4) {
            if chunk.len() == 4 {
                rgba_buffer.push(chunk[2]); // R
                rgba_buffer.push(chunk[1]); // G
                rgba_buffer.push(chunk[0]); // B
                rgba_buffer.push(chunk[3]); // A
            }
        }
        
        let rgba = image::RgbaImage::from_raw(width, height, rgba_buffer)
            .ok_or_else(|| anyhow!("Failed to create image from raw data"))?;
        
        let dynamic_image = DynamicImage::ImageRgba8(rgba);
        self.current_image = Some(dynamic_image);
        
        info!("Window captured: {}x{}", window_bounds.width, window_bounds.height);
        Ok(())
    }

    /// Get the current image
    pub fn get_current_image(&self) -> Option<&DynamicImage> {
        self.current_image.as_ref()
    }

    /// Get the current image as raw bytes
    pub fn get_current_image_data(&self) -> Result<Vec<u8>> {
        if let Some(image) = &self.current_image {
            let mut buffer = Vec::new();
            let mut cursor = Cursor::new(&mut buffer);
            image.write_to(&mut cursor, image::ImageOutputFormat::Png)?;
            Ok(buffer)
        } else {
            Err(anyhow!("No image available"))
        }
    }
}