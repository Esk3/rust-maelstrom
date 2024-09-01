use std::future::Future;

pub trait Service<Request> {
    type Response;
    type Future: Future<Output = anyhow::Result<Self::Response>>;

    fn call(&mut self, request: Request) -> Self::Future;
}
