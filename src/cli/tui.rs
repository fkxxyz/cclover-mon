use super::*;
pub fn run_tui(states: StateSource) -> std::io::Result<()> {
    use crossbeam_channel::TryRecvError;

    let receiver = states.subscribe();
    let mut ui = TerminalUi::enter()?;
    let mut state = receiver.recv().map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "monitor runtime stopped")
    })?;
    ui.draw(Dashboard::new(&state))?;

    loop {
        if ui.wait_for_quit(Duration::from_millis(100))? {
            return Ok(());
        }
        let mut redraw = ui.size_changed()?;
        loop {
            match receiver.try_recv() {
                Ok(next) => {
                    state = next;
                    redraw = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        if redraw {
            ui.draw(Dashboard::new(&state))?;
        }
    }
}
