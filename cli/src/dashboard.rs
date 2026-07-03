use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Gauge, List, ListItem, Paragraph, Sparkline,
    },
    Frame, Terminal,
};

use deadband_core::VitalSigns;

pub fn run_dashboard<F>(data_fn: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn() -> VitalSigns,
{
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = DashboardApp::new(data_fn);

    // Run the event loop
    let res = app.run(&mut terminal);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = res {
        eprintln!("Dashboard error: {}", e);
    }

    Ok(())
}

struct DashboardApp<F>
where
    F: Fn() -> VitalSigns,
{
    data_fn: F,
    tick_count: u64,
    history: Vec<(f64, f64)>, // (tick, loop_count) for sparkline
    should_quit: bool,
}

impl<F> DashboardApp<F>
where
    F: Fn() -> VitalSigns,
{
    fn new(data_fn: F) -> Self {
        Self {
            data_fn,
            tick_count: 0,
            history: Vec::with_capacity(120),
            should_quit: false,
        }
    }

    fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<(), Box<dyn std::error::Error>> {
        let tick_rate = Duration::from_millis(250);

        while !self.should_quit {
            terminal.draw(|f| self.ui(f))?;

            if event::poll(tick_rate)? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                        _ => {}
                    }
                }
            }

            // Record history (every other tick to spread out data)
            self.tick_count += 1;
            if self.tick_count % 2 == 0 {
                let vs = (self.data_fn)();
                self.history.push((self.tick_count as f64, vs.loop_count as f64));
                if self.history.len() > 120 {
                    self.history.remove(0);
                }
            }
        }

        Ok(())
    }

    fn ui(&self, frame: &mut Frame) {
        let vs = (self.data_fn)();

        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Header
                Constraint::Length(8),  // Top metrics row
                Constraint::Length(8),  // Detection breakdown row
                Constraint::Min(5),     // Sparkline chart
                Constraint::Length(3),  // Footer
            ])
            .split(frame.area());

        // Header
        let header = Block::default()
            .borders(Borders::ALL)
            .title(" Deadband Agent Vital Signs ")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        let header_text = Paragraph::new(Line::from(vec![
            Span::styled("q/ESC ", Style::default().fg(Color::DarkGray)),
            Span::raw("quit  |  "),
            Span::styled(format!("{} metrics collected", vs.total_events), Style::default().fg(Color::Green)),
        ]))
        .block(header);
        frame.render_widget(header_text, areas[0]);

        // Top metrics row (4 columns)
        let metric_areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .split(areas[1]);

        // Loop count gauge
        let loop_block = Block::default()
            .borders(Borders::ALL)
            .title(" Loops Detected ")
            .style(Style::default().fg(Color::Red));
        let loop_gauge = Gauge::default()
            .block(loop_block)
            .gauge_style(Style::default().fg(Color::Red).bg(Color::Black))
            .percent(vs.loop_count.min(100) as u16)
            .label(format!("{}", vs.loop_count));
        frame.render_widget(loop_gauge, metric_areas[0]);

        // Avg detection time
        let time_block = Block::default()
            .borders(Borders::ALL)
            .title(" Avg Detection Time ")
            .style(Style::default().fg(Color::Yellow));
        let time_gauge = Gauge::default()
            .block(time_block)
            .gauge_style(Style::default().fg(Color::Yellow).bg(Color::Black))
            .percent(((vs.avg_detection_time_ms / 100.0).min(1.0) * 100.0) as u16)
            .label(format!("{:.1}ms", vs.avg_detection_time_ms));
        frame.render_widget(time_gauge, metric_areas[1]);

        // Success rate
        let success_block = Block::default()
            .borders(Borders::ALL)
            .title(" Success Rate ")
            .style(Style::default().fg(Color::Green));
        let success_gauge = Gauge::default()
            .block(success_block)
            .gauge_style(Style::default().fg(Color::Green).bg(Color::Black))
            .percent((vs.success_rate * 100.0) as u16)
            .label(format!("{:.1}%", vs.success_rate * 100.0));
        frame.render_widget(success_gauge, metric_areas[2]);

        // API spend
        let spend_block = Block::default()
            .borders(Borders::ALL)
            .title(" API Spend (est.) ")
            .style(Style::default().fg(Color::Magenta));
        let spend_gauge = Gauge::default()
            .block(spend_block)
            .gauge_style(Style::default().fg(Color::Magenta).bg(Color::Black))
            .percent(((vs.api_spend / 10.0).min(1.0) * 100.0) as u16)
            .label(format!("${:.4}", vs.api_spend));
        frame.render_widget(spend_gauge, metric_areas[3]);

        // Detection breakdown
        let detect_areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(areas[2]);

        // Intervention breakdown
        let intv_block = Block::default()
            .borders(Borders::ALL)
            .title(" Interventions ")
            .style(Style::default().fg(Color::Blue));
        let intv_items: Vec<ListItem> = vs
            .intervention_breakdown
            .iter()
            .map(|(k, v)| {
                ListItem::new(format!(" {}: {}", k, v))
                    .style(Style::default().fg(Color::White))
            })
            .collect();
        let intv_list = List::new(intv_items).block(intv_block);
        frame.render_widget(intv_list, detect_areas[0]);

        // Detection kinds
        let detect_block = Block::default()
            .borders(Borders::ALL)
            .title(" Detection Kinds ")
            .style(Style::default().fg(Color::Yellow));
        let detect_items: Vec<ListItem> = vs
            .detection_breakdown
            .iter()
            .map(|(k, v)| {
                ListItem::new(format!(" {}: {}", k, v))
                    .style(Style::default().fg(Color::White))
            })
            .collect();
        let detect_list = List::new(detect_items).block(detect_block);
        frame.render_widget(detect_list, detect_areas[1]);

        // Loop sparkline chart
        let sparkline_block = Block::default()
            .borders(Borders::ALL)
            .title(" Loop Count Over Time ")
            .style(Style::default().fg(Color::Cyan));
        let data: Vec<u64> = self.history.iter().map(|(_, y)| *y as u64).collect();
        let sparkline = Sparkline::default()
            .block(sparkline_block)
            .data(&data)
            .style(Style::default().fg(Color::Red));
        frame.render_widget(sparkline, areas[3]);

        // Footer
        let footer = Block::default()
            .borders(Borders::ALL)
            .title(" Legend ")
            .style(Style::default().fg(Color::DarkGray));
        let footer_text = Paragraph::new(
            " Loops | Avg Detection Time | Success Rate | API Spend | Interventions | Detection Kinds",
        )
        .block(footer);
        frame.render_widget(footer_text, areas[4]);
    }
}

pub fn print_snapshot(vs: &VitalSigns) {
    println!("╔══════════════════════════════════════╗");
    println!("║     Deadband Agent Vital Signs       ║");
    println!("╠══════════════════════════════════════╣");
    println!("║  Loops Detected:      {:>6}        ║", vs.loop_count);
    println!("║  Avg Detection Time:  {:>6.1}ms     ║", vs.avg_detection_time_ms);
    println!("║  Success Rate:        {:>5.1}%      ║", vs.success_rate * 100.0);
    println!("║  API Spend (est.):    ${:>8.4}  ║", vs.api_spend);
    println!("║  Total Events:        {:>6}        ║", vs.total_events);
    println!("║  Interventions:       {:>6}        ║", vs.total_interventions);
    println!("╠══════════════════════════════════════╣");
    println!("║  Detection Breakdown:                ║");
    for (kind, count) in &vs.detection_breakdown {
        println!("║    {:20} {:>6}     ║", format!("{}:", kind), count);
    }
    println!("╚══════════════════════════════════════╝");
}
