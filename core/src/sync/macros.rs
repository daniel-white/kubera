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

pub enum ReadyState<T> {
    Ready(T),
    NotReady,
}

#[macro_export]
macro_rules! await_ready {
    // Single receiver
    ($r:ident) => {
        match $r.get().await.as_ref() {
            Some(val) => ReadyState::Ready(val),
            None => ReadyState::NotReady,
        }
    };
    // Two receivers
    ($r1:ident, $r2:ident) => {
        match ($r1.get().await.as_ref(), $r2.get().await.as_ref()) {
            (Some(val1), Some(val2)) => ReadyState::Ready((val1, val2)),
            _ => ReadyState::NotReady,
        }
    };
    // Three receivers
    ($r1:ident, $r2:ident, $r3:ident) => {
        match (
            $r1.get().await.as_ref(),
            $r2.get().await.as_ref(),
            $r3.get().await.as_ref(),
        ) {
            (Some(val1), Some(val2), Some(val3)) => ReadyState::Ready((val1, val2, val3)),
            _ => ReadyState::NotReady,
        }
    };
    // Four receivers
    ($r1:ident, $r2:ident, $r3:ident, $r4:ident) => {
        match (
            $r1.get().await.as_ref(),
            $r2.get().await.as_ref(),
            $r3.get().await.as_ref(),
            $r4.get().await.as_ref(),
        ) {
            (Some(val1), Some(val2), Some(val3), Some(val4)) => {
                ReadyState::Ready((val1, val2, val3, val4))
            }
            _ => ReadyState::NotReady,
        }
    };
    // Five receivers
    ($r1:ident, $r2:ident, $r3:ident, $r4:ident, $r5:ident) => {
        match (
            $r1.get().await.as_ref(),
            $r2.get().await.as_ref(),
            $r3.get().await.as_ref(),
            $r4.get().await.as_ref(),
            $r5.get().await.as_ref(),
        ) {
            (Some(val1), Some(val2), Some(val3), Some(val4), Some(val5)) => {
                ReadyState::Ready((val1, val2, val3, val4, val5))
            }
            _ => ReadyState::NotReady,
        }
    };
    // Six receivers
    ($r1:ident, $r2:ident, $r3:ident, $r4:ident, $r5:ident, $r6:ident) => {
        match (
            $r1.get().await.as_ref(),
            $r2.get().await.as_ref(),
            $r3.get().await.as_ref(),
            $r4.get().await.as_ref(),
            $r5.get().await.as_ref(),
            $r6.get().await.as_ref(),
        ) {
            (Some(val1), Some(val2), Some(val3), Some(val4), Some(val5), Some(val6)) => {
                ReadyState::Ready((val1, val2, val3, val4, val5, val6))
            }
            _ => ReadyState::NotReady,
        }
    };
    // Seven receivers
    ($r1:ident, $r2:ident, $r3:ident, $r4:ident, $r5:ident, $r6:ident, $r7:ident) => {
        match (
            $r1.get().await.as_ref(),
            $r2.get().await.as_ref(),
            $r3.get().await.as_ref(),
            $r4.get().await.as_ref(),
            $r5.get().await.as_ref(),
            $r6.get().await.as_ref(),
            $r7.get().await.as_ref(),
        ) {
            (
                Some(val1),
                Some(val2),
                Some(val3),
                Some(val4),
                Some(val5),
                Some(val6),
                Some(val7),
            ) => ReadyState::Ready((val1, val2, val3, val4, val5, val6, val7)),
            _ => ReadyState::NotReady,
        }
    };
    // Eight receivers
    ($r1:ident, $r2:ident, $r3:ident, $r4:ident, $r5:ident, $r6:ident, $r7:ident, $r8:ident) => {
        match (
            $r1.get().await.as_ref(),
            $r2.get().await.as_ref(),
            $r3.get().await.as_ref(),
            $r4.get().await.as_ref(),
            $r5.get().await.as_ref(),
            $r6.get().await.as_ref(),
            $r7.get().await.as_ref(),
            $r8.get().await.as_ref(),
        ) {
            (
                Some(val1),
                Some(val2),
                Some(val3),
                Some(val4),
                Some(val5),
                Some(val6),
                Some(val7),
                Some(val8),
            ) => ReadyState::Ready((val1, val2, val3, val4, val5, val6, val7, val8)),
            _ => ReadyState::NotReady,
        }
    };
    // Nine receivers
    ($r1:ident, $r2:ident, $r3:ident, $r4:ident, $r5:ident, $r6:ident, $r7:ident, $r8:ident, $r9:ident) => {
        match (
            $r1.get().await.as_ref(),
            $r2.get().await.as_ref(),
            $r3.get().await.as_ref(),
            $r4.get().await.as_ref(),
            $r5.get().await.as_ref(),
            $r6.get().await.as_ref(),
            $r7.get().await.as_ref(),
            $r8.get().await.as_ref(),
            $r9.get().await.as_ref(),
        ) {
            (
                Some(val1),
                Some(val2),
                Some(val3),
                Some(val4),
                Some(val5),
                Some(val6),
                Some(val7),
                Some(val8),
                Some(val9),
            ) => ReadyState::Ready((val1, val2, val3, val4, val5, val6, val7, val8, val9)),
            _ => ReadyState::NotReady,
        }
    };
}
