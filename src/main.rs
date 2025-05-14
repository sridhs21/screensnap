// src/main.rs
use anyhow::Result;
use clap::{Parser, Subcommand};
use log::{info, error, warn};
use image::ImageFormat;
use std::path::PathBuf;
use std::io::BufRead;
use crate::ai::connector::AiConnector;

mod capture;
mod ai;
mod gui; // GUI module

#[derive(Parser)]
#[command(name = "screensnap")]
#[command(about = "Screenshot AI tool with local Ollama support", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Capture and analyze a screenshot with local Ollama
    Capture {
        /// Ollama model name (e.g., "llava:latest")
        #[arg(long, short = 'm')]
        model: Option<String>,
        
        /// Ollama server URL (default: http://localhost:11434)
        #[arg(long)]
        ollama_url: Option<String>,
        
        /// Save screenshot to file
        #[arg(long)]
        save: Option<PathBuf>,
        
        /// Window title to capture (optional)
        #[arg(long)]
        window: Option<String>,
        
        /// Skip AI analysis - just capture and save
        #[arg(long)]
        no_ai: bool,
    },
    /// List available windows
    ListWindows,
    /// List available Ollama models
    ListModels {
        /// Ollama server URL (default: http://localhost:11434)
        #[arg(long)]
        ollama_url: Option<String>,
    },
    /// Pull an Ollama model
    PullModel {
        /// Model name to pull (e.g., "llava:latest")
        model: String,
        
        /// Ollama server URL (default: http://localhost:11434)
        #[arg(long)]
        ollama_url: Option<String>,
    },
    /// Check Ollama status
    CheckOllama {
        /// Ollama server URL (default: http://localhost:11434)
        #[arg(long)]
        ollama_url: Option<String>,
    },
    /// Run simple interactive mode
    Interactive,
    /// Run graphical user interface
    Gui,
}

fn main() -> Result<()> {
    // Initialize logging
    env_logger::init_from_env(
        env_logger::Env::default().filter_or("RUST_LOG", "info")
    );

    let cli = Cli::parse();
    
    match cli.command {
        Commands::Capture { model, ollama_url, save, window, no_ai } => {
            run_capture_cli(model, ollama_url, save, window, no_ai)
        }
        Commands::ListWindows => {
            list_windows()
        }
        Commands::ListModels { ollama_url } => {
            list_ollama_models(ollama_url)
        }
        Commands::PullModel { model, ollama_url } => {
            pull_ollama_model(model, ollama_url)
        }
        Commands::CheckOllama { ollama_url } => {
            check_ollama_status(ollama_url)
        }
        Commands::Interactive => {
            run_interactive_mode()
        }
        Commands::Gui => {
            // Run the new GUI mode
            gui::run_gui()
        }
    }
}

fn run_capture_cli(model: Option<String>, ollama_url: Option<String>, save: Option<PathBuf>, window: Option<String>, no_ai: bool) -> Result<()> {
    info!("Starting headless capture mode");
    
    // Initialize screenshot manager
    let mut screenshot_manager = capture::screenshot::ScreenshotManager::new()?;
    
    // Capture screenshot
    if let Some(window_title) = window {
        info!("Capturing window: {}", window_title);
        match screenshot_manager.capture_window(&window_title) {
            Ok(_) => info!("Window captured successfully"),
            Err(e) => {
                error!("Failed to capture window '{}': {}", window_title, e);
                warn!("Falling back to full screen capture...");
                screenshot_manager.capture_screen()?;
            }
        }
    } else {
        info!("Capturing full screen");
        screenshot_manager.capture_screen()?;
    }
    
    // Save if requested
    if let Some(save_path) = &save {
        if let Some(image) = screenshot_manager.get_current_image() {
            image.save_with_format(save_path, ImageFormat::Png)?;
            info!("Screenshot saved to: {}", save_path.display());
        }
    }
    
    // Process with AI if requested
    if !no_ai {
        let model_name = model.unwrap_or_else(|| "llava:latest".to_string());
        let url = get_ollama_url(ollama_url);
        
        info!("Processing with Ollama model: {} at {}", model_name, url);
        
        // Set Ollama URL as environment variable
        std::env::set_var("OLLAMA_HOST", &url);
        
        // Initialize Ollama model
        match ai::local_model::LocalModel::new(&model_name) {
            Ok(mut ai_model) => {
                // Get image data
                match screenshot_manager.get_current_image_data() {
                    Ok(image_data) => {
                        // Process with AI
                        match ai_model.process_image(&image_data) {
                            Ok(response) => {
                                println!("\n=== AI Analysis (Ollama: {}) ===", model_name);
                                println!("{}", response);
                                println!("===========================================\n");
                            }
                            Err(e) => {
                                error!("AI processing failed: {}", e);
                                
                                if e.to_string().contains("not found") {
                                    println!("\nTo fix this, run:");
                                    println!("  ollama pull {}", model_name);
                                } else if e.to_string().contains("not available") {
                                    println!("\nTo fix this, run:");
                                    println!("  ollama serve");
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to get image data: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("Failed to initialize Ollama model: {}", e);
                println!("\nMake sure Ollama is running: ollama serve");
                println!("And that the model is available: ollama pull {}", model_name);
            }
        }
    }
    
    Ok(())
}

fn list_windows() -> Result<()> {
    info!("Listing available windows...");
    
    match capture::window_finder::get_window_titles() {
        Ok(windows) => {
            println!("\nAvailable windows:");
            for (i, window) in windows.iter().enumerate() {
                println!("  {}. {}", i + 1, window);
            }
            println!();
        }
        Err(e) => {
            error!("Failed to get window list: {}", e);
        }
    }
    
    Ok(())
}

fn get_ollama_url(url_arg: Option<String>) -> String {
    url_arg.unwrap_or_else(|| {
        std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string())
    })
}

fn list_ollama_models(ollama_url: Option<String>) -> Result<()> {
    let url = get_ollama_url(ollama_url);
    info!("Listing Ollama models at {}...", url);
    
    let client = reqwest::blocking::Client::new();
    let api_url = format!("{}/api/tags", url);
    
    match client.get(&api_url).send() {
        Ok(response) => {
            if response.status().is_success() {
                let data: serde_json::Value = response.json()?;
                
                println!("\nAvailable models:");
                if let Some(models) = data["models"].as_array() {
                    for model in models {
                        if let Some(name) = model["name"].as_str() {
                            let size = model["size"].as_i64().unwrap_or(0);
                            let size_gb = size as f64 / (1024.0 * 1024.0 * 1024.0);
                            println!("  - {} ({:.1} GB)", name, size_gb);
                        }
                    }
                } else {
                    println!("  No models found");
                }
                println!();
                
                println!("Suggested vision models for screenshots:");
                println!("  - llava:latest (general vision model)");
                println!("  - llava:13b (larger, more accurate)");
                println!("  - llava:7b (smaller, faster)");
            } else {
                error!("Ollama server error: {}", response.status());
            }
        }
        Err(e) => {
            error!("Failed to connect to Ollama: {}", e);
            println!("\nMake sure Ollama is running: ollama serve");
        }
    }
    
    Ok(())
}

fn pull_ollama_model(model: String, ollama_url: Option<String>) -> Result<()> {
    let url = get_ollama_url(ollama_url);
    info!("Pulling model {} from {}...", model, url);
    
    let client = reqwest::blocking::Client::new();
    let api_url = format!("{}/api/pull", url);
    
    let request = serde_json::json!({
        "name": model,
        "stream": true
    });
    
    println!("Pulling model {}...", model);
    println!("This may take a while depending on the model size and your internet connection.");
    
    match client.post(&api_url).json(&request).send() {
        Ok(response) => {
            if response.status().is_success() {
                println!("Model {} pulled successfully!", model);
            } else {
                error!("Failed to pull model: {}", response.text()?);
            }
        }
        Err(e) => {
            error!("Failed to connect to Ollama: {}", e);
        }
    }
    
    Ok(())
}

fn check_ollama_status(ollama_url: Option<String>) -> Result<()> {
    let url = get_ollama_url(ollama_url);
    info!("Checking Ollama status at {}...", url);
    
    let client = reqwest::blocking::Client::new();
    let api_url = format!("{}/api/tags", url);
    
    match client.get(&api_url).send() {
        Ok(response) => {
            if response.status().is_success() {
                println!("✓ Ollama is running at {}", url);
                
                let data: serde_json::Value = response.json()?;
                if let Some(models) = data["models"].as_array() {
                    println!("✓ {} model(s) available", models.len());
                }
            } else {
                println!("✗ Ollama server error: {}", response.status());
            }
        }
        Err(e) => {
            println!("✗ Could not connect to Ollama at {}", url);
            println!("  Error: {}", e);
            println!("\nTroubleshooting:");
            println!("  1. Install Ollama: https://ollama.ai");
            println!("  2. Start Ollama: ollama serve");
            println!("  3. Pull a vision model: ollama pull llava:latest");
        }
    }
    
    Ok(())
}

fn run_interactive_mode() -> Result<()> {
    use std::io::{self, Write};
    
    println!("🖼️  ScreenSnap Interactive Mode");
    println!("===============================");
    println!();
    
    // Initialize the application
    let model_name = "llava:latest".to_string();
    
    // Initialize screenshot manager
    let mut screenshot_manager = capture::screenshot::ScreenshotManager::new()?;
    
    let stdin = io::stdin();
    let mut input = String::new();
    
    // Main menu loop
    loop {
        println!("\nMain Menu:");
        println!("1. Capture Full Screen");
        println!("2. Capture Specific Window");
        println!("3. List Available Models");
        println!("4. Exit");
        print!("\nEnter your choice (1-4): ");
        io::stdout().flush()?;
        
        input.clear();
        stdin.lock().read_line(&mut input)?;
        let choice = input.trim();
        
        match choice {
            "1" => {
                println!("\nCapturing full screen...");
                match screenshot_manager.capture_screen() {
                    Ok(_) => {
                        println!("✓ Screen captured successfully");
                        process_screenshot(&mut screenshot_manager, &model_name)?;
                    },
                    Err(e) => {
                        println!("✗ Failed to capture screen: {}", e);
                    }
                }
            },
            "2" => {
                match list_windows() {
                    Ok(_) => {
                        print!("Enter window number or name to capture (or leave empty to cancel): ");
                        io::stdout().flush()?;
                        
                        input.clear();
                        stdin.lock().read_line(&mut input)?;
                        let window_choice = input.trim();
                        
                        if !window_choice.is_empty() {
                            println!("\nCapturing window: {}...", window_choice);
                            
                            // Try to capture by number first
                            let window_title = if let Ok(num) = window_choice.parse::<usize>() {
                                if let Ok(windows) = capture::window_finder::get_window_titles() {
                                    if num > 0 && num <= windows.len() {
                                        Some(windows[num - 1].clone())
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                Some(window_choice.to_string())
                            };
                            
                            if let Some(title) = window_title {
                                match screenshot_manager.capture_window(&title) {
                                    Ok(_) => {
                                        println!("✓ Window captured successfully");
                                        process_screenshot(&mut screenshot_manager, &model_name)?;
                                    },
                                    Err(e) => {
                                        println!("✗ Failed to capture window: {}", e);
                                        println!("Falling back to full screen capture...");
                                        
                                        if let Err(e) = screenshot_manager.capture_screen() {
                                            println!("✗ Full screen capture also failed: {}", e);
                                        } else {
                                            println!("✓ Full screen captured instead");
                                            process_screenshot(&mut screenshot_manager, &model_name)?;
                                        }
                                    }
                                }
                            } else {
                                println!("Invalid window number or name");
                            }
                        }
                    },
                    Err(e) => {
                        println!("✗ Failed to list windows: {}", e);
                    }
                }
            },
            "3" => {
                list_ollama_models(None)?;
            },
            "4" => {
                println!("Exiting ScreenSnap");
                break;
            },
            _ => {
                println!("Invalid choice. Please enter a number between 1 and 4.");
            }
        }
    }
    
    Ok(())
}

fn process_screenshot(screenshot_manager: &mut capture::screenshot::ScreenshotManager, model_name: &str) -> Result<()> {
    use std::io::{self, Write};
    
    // Get the image data
    match screenshot_manager.get_current_image_data() {
        Ok(image_data) => {
            // Save options
            println!("\nScreenshot Options:");
            println!("1. Analyze with AI ({})", model_name);
            println!("2. Save to file");
            println!("3. Both analyze and save");
            println!("4. Return to main menu");
            print!("\nEnter your choice (1-4): ");
            io::stdout().flush()?;
            
            let mut input = String::new();
            io::stdin().lock().read_line(&mut input)?;
            let choice = input.trim();
            
            let analyze = matches!(choice, "1" | "3");
            let save = matches!(choice, "2" | "3");
            
            // Process with AI if requested
            if analyze {
                println!("\nAnalyzing screenshot with {}...", model_name);
                
                // Set Ollama URL as environment variable
                std::env::set_var("OLLAMA_HOST", &get_ollama_url(None));
                
                // Initialize Ollama model
                match ai::local_model::LocalModel::new(model_name) {
                    Ok(mut ai_model) => {
                        // Process with AI
                        println!("Sending image to Ollama for analysis...");
                        println!("This may take a moment depending on your system...");
                        match ai_model.process_image(&image_data) {
                            Ok(response) => {
                                println!("\n=== AI Analysis ({}) ===", model_name);
                                println!("{}", response);
                                println!("===========================================\n");
                            }
                            Err(e) => {
                                error!("AI processing failed: {}", e);
                                
                                if e.to_string().contains("not found") {
                                    println!("\nTo fix this, run:");
                                    println!("  ollama pull {}", model_name);
                                } else if e.to_string().contains("not available") {
                                    println!("\nTo fix this, run:");
                                    println!("  ollama serve");
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to initialize Ollama model: {}", e);
                        println!("\nMake sure Ollama is running: ollama serve");
                        println!("And that the model is available: ollama pull {}", model_name);
                    }
                }
            }
            
            // Save if requested
            if save {
                print!("Enter filename to save (e.g., screenshot.png): ");
                io::stdout().flush()?;
                
                input.clear();
                io::stdin().lock().read_line(&mut input)?;
                let filename = input.trim();
                
                if !filename.is_empty() {
                    if let Some(image) = screenshot_manager.get_current_image() {
                        let path = std::path::Path::new(filename);
                        image.save_with_format(path, ImageFormat::Png)?;
                        println!("✓ Screenshot saved to: {}", filename);
                    }
                }
            }
        }
        Err(e) => {
            println!("✗ Failed to get image data: {}", e);
        }
    }
    
    Ok(())
}