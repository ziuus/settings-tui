sed -i -e '/let app_clone_sponsor = app.clone();/a \    let api_url = cfg.sponsor_api_url.clone().unwrap_or_else(|| "https://settings-tui.vercel.app/api/sponsor".to_string());\
' src/app/mod.rs
sed -i 's/client.get("https:\/\/settings-tui.vercel.app\/api\/sponsor")/client.get(\&api_url)/' src/app/mod.rs
