sed -i '/let sponsor_message = std::sync::Arc::new(tokio::sync::Mutex::new(None::<String>));/d' src/app/mod.rs
sed -i '/let sponsor_clone = sponsor_message.clone();/d' src/app/mod.rs
sed -i 's/\*sponsor_clone.lock().await = Some(msg.to_string());/app_clone.lock().await.sponsor_msg = Some(msg.to_string());/' src/app/mod.rs
