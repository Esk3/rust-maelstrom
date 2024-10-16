use serde::de::DeserializeOwned;

use crate::Fut;

use super::Service;

#[derive(Debug)]
pub struct JsonLayer<S, Req>(S, std::marker::PhantomData<Req>);

impl<S, Res> JsonLayer<S, Res> {
    pub fn new(s: S) -> Self {
        Self(s, std::marker::PhantomData)
    }
}

impl<S, Req> Service<String> for JsonLayer<S, Req>
where
    S: Service<Req> + Clone + Send + 'static,
    Req: DeserializeOwned,
{
    type Response = S::Response;

    type Future = Fut<Self::Response>;

    fn call(&mut self, request: String) -> Self::Future {
        let mut this = self.clone();
        Box::pin(async move {
            let request = serde_json::from_str(&request)?;
            this.0.call(request).await
        })
    }
}

impl<S, Req> Clone for JsonLayer<S, Req>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1)
    }
}
