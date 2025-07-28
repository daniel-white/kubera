use std::cell::RefCell;
use std::rc::Rc;
use tokio::task::JoinSet;
use tracing::error;

type MutableJoinSet = Rc<RefCell<JoinSet<()>>>;

#[derive(Default)]
pub struct Builder {
    join_set: MutableJoinSet,
}

impl Builder {
    pub fn new_task(&self, name: &'static str) -> Spawner {
        Spawner {
            name,
            join_set: self.join_set.clone(),
        }
    }

    pub async fn join_all(self) {
        let join_set = self.join_set.take();
        let _ = join_set.join_all().await;
    }
}

pub struct Spawner {
    name: &'static str,
    join_set: MutableJoinSet,
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
}
