use std::cell::RefCell;
use std::rc::Rc;
use tokio::task::JoinSet;
use tokio_shutdown::Shutdown;
use tracing::error;

type MutableJoinSet = Rc<RefCell<JoinSet<()>>>;

pub struct Builder {
    join_set: MutableJoinSet,
    shutdown: Shutdown,
}

impl Builder {
    pub fn new_task(&self, name: &'static str) -> Spawner {
        Spawner {
            name,
            join_set: self.join_set.clone(),
            shutdown: self.shutdown.clone(),
        }
    }

    pub async fn join_all(self) {
        let join_set = self.join_set.take();
        let _ = join_set.join_all().await;
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            join_set: MutableJoinSet::default(),
            shutdown: Shutdown::new().expect("Failed to create shutdown handle"),
        }
    }
}

pub struct Spawner {
    name: &'static str,
    join_set: MutableJoinSet,
    shutdown: Shutdown,
}

impl Spawner {
    #[track_caller]
    pub fn spawn<F>(self, task: F)
    where
        F: Future<Output = ()>,
        F: Send + 'static,
    {
        let result = self
            .join_set
            .borrow_mut()
            .build_task()
            .name(self.name)
            .spawn(task);

        if let Err(err) = result {
            error!("Failed to spawn task '{}': {}", self.name, err);
        }
    }

    #[track_caller]
    pub fn spawn_blocking<F>(self, task: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let result = self
            .join_set
            .borrow_mut()
            .build_task()
            .name(self.name)
            .spawn_blocking(task);

        if let Err(err) = result {
            error!("Failed to spawn blocking task '{}': {}", self.name, err);
        }
    }

    #[track_caller]
    pub fn spawn_on_shutdown<F>(self, task: F)
    where
        F: Future<Output = ()>,
        F: Send + 'static,
    {
        let shutdown = self.shutdown.clone();
        self.spawn(async move {
            let () = shutdown.handle().await;
            task.await;
        });
    }
}
