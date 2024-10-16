use crate::{message::Message, Fut};

use super::Service;

#[derive(Debug)]
pub struct MessageLayer<S, Req>(S, std::marker::PhantomData<Req>);

impl<S, Req> Service<Message<Req>> for MessageLayer<S, Req>
where
    S: Service<Req> + Clone + Send + 'static,
    Req: Send + 'static,
{
    type Response = Message<S::Response>;

    type Future = Fut<Self::Response>;

    fn call(&mut self, request: Message<Req>) -> Self::Future {
        let mut this = self.clone();
        Box::pin(async move {
            let (reply, body) = request.into_reply();
            let response = this.0.call(body).await?;
            Ok(reply.with_body(response))
        })
    }
}

impl<S, Req> Clone for MessageLayer<S, Req>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1)
    }
}
