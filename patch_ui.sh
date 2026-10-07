sed -i -e '/let main_layout = Layout::default()/,+10c\
    let mut constraints = vec![\
        Constraint::Length(1), // Top bar space\
        Constraint::Length(1), // Header\
        Constraint::Length(1), // Spacer\
        Constraint::Min(0),    // Main content\
    ];\
    if app.sponsor_msg.is_some() {\
        constraints.push(Constraint::Length(1)); // Sponsor ad spacer\
        constraints.push(Constraint::Length(1)); // Sponsor ad\
    }\
    constraints.push(Constraint::Length(1)); // Footer spacer\
    constraints.push(Constraint::Length(1)); // Footer keybindings\
\
    let main_layout = Layout::default()\
        .direction(Direction::Vertical)\
        .constraints(constraints)\
        .split(area.inner(&Margin {\
            vertical: 1,\
            horizontal: 2,\
        }));\
' src/ui/mod.rs
