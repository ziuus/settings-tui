sed -i -e '/f.render_widget(footer, main_layout\[5\]);/c\
    f.render_widget(footer, main_layout[main_layout.len() - 1]);\
\
    if let Some(msg) = &app.sponsor_msg {\
        let sponsor_p = Paragraph::new(Line::from(vec![\
            Span::styled(msg, Style::default().fg(Color::DarkGray)),\
            Span::styled(" [Ad]", Style::default().fg(Color::DarkGray)),\
        ])).alignment(ratatui::layout::Alignment::Right);\
        f.render_widget(sponsor_p, main_layout[main_layout.len() - 3]);\
    }\
' src/ui/mod.rs
