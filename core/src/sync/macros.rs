#[macro_export]
macro_rules! continue_after {
    ($duration:expr) => {{
        use tokio::signal::ctrl_c;
        use tokio::time;
        use tracing::debug;

        #[allow(clippy::needless_continue)]

        let sleep = time::sleep($duration);
        tokio::pin!(sleep);

        tokio::select! {
                _ = &mut sleep => {
                    debug!("Sleep duration of {:?} elapsed, continuing loop", $duration);
                    continue;
                },
            _ = ctrl_c() => {
                debug!("Interrupt signal received, exiting loop");
                break;
            }
        };
    }};

    ($duration:expr, $($fut:expr),+ $(,)?) => {{
        use tokio::signal::ctrl_c;
        use tokio::time;
        use tracing::debug;

        #[allow(clippy::needless_continue)]

        let sleep = time::sleep($duration);
        tokio::pin!(sleep);

        tokio::select! {
            _ = &mut sleep => {
                debug!("Sleep duration of {:?} elapsed, continuing loop", $duration);
                continue;
            },
            $(res = $fut => {
                match res {
                    Ok(_) =>{
                        debug!("Future {} advanced, continuing loop", stringify!($fut));
                        continue;
                    }
                    Err(e) => {
                        debug!("Future {} advanced with err, exiting loop: {:?}", stringify!($fut), e);
                        break;
                    }
                }
            },)+
            _ = ctrl_c() => {
                debug!("Interrupt signal received, exiting loop");
                break;
            }
        };
    }};
}

#[macro_export]
macro_rules! continue_on {
    ($($fut:expr),+ $(,)?) => {{
        use tokio::signal::ctrl_c;
        use tracing::debug;

        #[allow(clippy::needless_continue)]

        tokio::select! {
            $(
                res = $fut => {
                    match res {
                        Ok(_) =>{
                            debug!("Future {} advanced, continuing loop", stringify!($fut));
                            continue;
                        }
                        Err(e) => {
                            debug!("Future {} advanced with err, exiting loop: {:?}", stringify!($fut), e);
                            break;
                        }
                    }
                },
            )+
            _ = ctrl_c() => {
                debug!("Interrupt signal received, exiting loop");
                break;
            }
        };
    }};
}
