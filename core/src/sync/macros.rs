#[macro_export]
macro_rules! select_continue {
    ($($fut:expr),+ $(,)?) => {{
        use tokio::signal::ctrl_c;
        use tracing::debug;

        tokio::select! {
            $(
                res = $fut => {
                    match res {
                        Ok(_) =>{
                            debug!("Future {} advanced, continuing loop", stringify!($fut));
                            continue;
                        }
                        Err(_) => {
                            debug!("Future {} advanced with err, exiting loop", stringify!($fut));
                            break;
                        }
                    }
                },
            )+
            _ = ctrl_c() => {
                debug!("Interupt signal received, exiting loop");
                break;
            }
        };
    }};
}
