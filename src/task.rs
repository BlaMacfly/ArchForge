//! Travaux en arrière-plan : chaque requête à MariaDB part sur son propre fil, pour que l'interface ne gèle
//! jamais ; le résultat revient par un canal, et l'interface est redessinée à son arrivée.

use anyhow::Result;
use std::sync::mpsc::{channel, Receiver, TryRecvError};

pub struct Task<T> {
    rx: Receiver<Result<T>>,
}

impl<T: Send + 'static> Task<T> {
    pub fn spawn(ctx: &egui::Context, job: impl FnOnce() -> Result<T> + Send + 'static) -> Self {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job))
                .unwrap_or_else(|_| Err(anyhow::anyhow!("erreur interne pendant la requête")));
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        Task { rx }
    }

    /// Résultat, s'il est arrivé.
    pub fn poll(&self) -> Option<Result<T>> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(anyhow::anyhow!("requête interrompue"))),
        }
    }
}

/// Emplacement d'un travail en cours : `Slot::poll` rend le résultat une seule fois.
pub struct Slot<T> {
    task: Option<Task<T>>,
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Slot { task: None }
    }
}

impl<T: Send + 'static> Slot<T> {
    pub fn start(&mut self, ctx: &egui::Context, job: impl FnOnce() -> Result<T> + Send + 'static) {
        self.task = Some(Task::spawn(ctx, job));
    }

    pub fn busy(&self) -> bool {
        self.task.is_some()
    }

    pub fn poll(&mut self) -> Option<Result<T>> {
        let r = self.task.as_ref()?.poll()?;
        self.task = None;
        Some(r)
    }
}
