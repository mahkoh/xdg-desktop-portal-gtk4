use {
    crate::portal::response::Response,
    async_channel::Sender,
    error_reporter::Report,
    futures_util::{select, FutureExt},
    std::future::Future,
    zbus::{
        interface,
        zvariant::{OwnedObjectPath, Type},
        ObjectServer,
    },
};

async fn export_request(server: &ObjectServer, path: OwnedObjectPath) {
    let (send, recv) = async_channel::bounded(1);
    if let Err(e) = server.at(&path, Request { send }).await {
        log::error!("Could not export request object: {}", Report::new(e));
        return;
    }
    let _ = recv.recv().await;
    let _ = server.remove::<Request, _>(&path).await;
}

/// Runs the future to completion or exits early if the request is closed.
///
/// This is inherently racy because the request might get cancelled before we export the
/// path.
pub async fn run_request<T, F>(server: &ObjectServer, handle: OwnedObjectPath, f: F) -> Response<T>
where
    T: Default + Type,
    F: Future<Output = Response<T>>,
{
    select! {
        v = f.fuse() => v,
        _ = export_request(server, handle).fuse() => Response::cancelled(),
    }
}

struct Request {
    send: Sender<()>,
}
#[interface(name = "org.freedesktop.impl.portal.Request")]
impl Request {
    async fn close(&self) {
        let _ = self.send.send(()).await;
    }
}
