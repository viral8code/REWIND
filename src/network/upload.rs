use super::*;
use std::sync::Mutex as SyncMutex;
use tokio::sync::mpsc as queue;
pub(super) struct Upload {
    data: Mutex<UploadData>,
    cancel: SyncMutex<Option<oneshot::Sender<()>>>,
    busy: AtomicBool,
    closed: AtomicBool,
    deadline: tokio::time::Instant,
    limit: usize,
    response_limit: usize,
}
struct UploadData {
    sender: Option<queue::Sender<bytes::Bytes>>,
    response: Option<oneshot::Receiver<Value>>,
    total: usize,
}
impl Upload {
    pub fn cancel_driver(&self) {
        if let Some(cancel) = self.cancel.lock().unwrap().take() {
            let _ = cancel.send(());
        }
    }
    pub fn reserved(&self) -> usize {
        response_reservation(self.response_limit) + 2 * CHUNK + 4096
    }
}
impl Host {
    pub(super) fn submit_upload(
        &mut self,
        id: usize,
        request: Request,
        client: reqwest::Client,
    ) -> Result<(), &'static str> {
        if self.uploads.len() >= 8 {
            return Err("HttpUploadLimit");
        }
        let (chunks, incoming) = queue::channel(1);
        let (result, response) = oneshot::channel();
        let (cancel, cancelled) = oneshot::channel();
        let slot = Arc::new(Upload {
            data: Mutex::new(UploadData {
                sender: Some(chunks),
                response: Some(response),
                total: 0,
            }),
            cancel: SyncMutex::new(Some(cancel)),
            busy: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            deadline: tokio::time::Instant::now() + Duration::from_millis(request.timeout_ms),
            limit: request.upload_limit.unwrap(),
            response_limit: request.limit,
        });
        let response_limit = request.limit;
        let phase = Arc::new(AtomicU8::new(0));
        let worker_phase = phase.clone();
        let worker = slot.clone();
        self.runtime.spawn(async move {
            let body=reqwest::Body::wrap_stream(futures_util::stream::unfold(incoming,|mut receiver|async move{receiver.recv().await.map(|bytes|(Ok::<_,std::io::Error>(bytes),receiver))}));
            let future=async {if worker_phase.compare_exchange(0,1,Ordering::SeqCst,Ordering::SeqCst).is_err(){return failure("HttpCancelled","NotSent",0);}execute_with_body(client,request,&worker_phase,None,Some(body)).await};
            let outcome=tokio::select!{biased;_=cancelled=>failure("HttpCancelled",if worker_phase.load(Ordering::SeqCst)==0{"NotSent"}else{"Unknown"},0),v=future=>v};
            let _=result.send(outcome);worker.closed.store(true,Ordering::SeqCst);
        });
        self.uploads.insert(id, slot);
        let (sender, receiver) = mpsc::channel();
        let (cancel, _) = oneshot::channel();
        let _ = sender.send(json!({"upload":id,"maxBytes":response_limit}));
        self.jobs.insert(
            id,
            Job {
                receiver,
                cancel: Some(cancel),
                phase,
                bytes: 4096,
                resource: None,
                finishing: false,
            },
        );
        Ok(())
    }
    pub(super) fn write_upload(
        &mut self,
        id: usize,
        stream: usize,
        body: Arc<Vec<u8>>,
    ) -> Result<(), &'static str> {
        if body.len() > CHUNK {
            return Err("HttpChunkLimit");
        }
        if self.jobs.len() >= MAX_JOBS {
            return Err("HttpConcurrencyLimit");
        }
        let slot = self.uploads.get(&stream).ok_or("HttpUploadClosed")?.clone();
        if slot.closed.load(Ordering::SeqCst) {
            return Err("HttpUploadClosed");
        }
        if slot.busy.swap(true, Ordering::SeqCst) {
            return Err("HttpUploadBusy");
        }
        let (sender, receiver) = mpsc::channel();
        let (cancel, cancelled) = oneshot::channel();
        self.runtime.spawn(async move {
            let future=async {
                let mut data=slot.data.lock().await;
                if body.len()>slot.limit.saturating_sub(data.total){return failure("HttpBodyLimit","Unknown",0);}
                let Some(queue)=&data.sender else{return failure("HttpUploadClosed","Unknown",0);};
                let count=body.len();
                match queue.send(bytes::Bytes::from_owner(SharedBody(body))).await {Ok(())=>{data.total+=count;json!({"written":count,"phase":"Unknown"})},Err(_)=>failure("HttpUploadClosed","Unknown",0)}
            };
            let outcome=tokio::select!{biased;_=cancelled=>failure("HttpCancelled","Unknown",0),v=tokio::time::timeout_at(slot.deadline,future)=>v.unwrap_or_else(|_|failure("HttpTimeout","Unknown",0))};
            slot.busy.store(false,Ordering::SeqCst);let _=sender.send(outcome);
        });
        self.jobs.insert(
            id,
            Job {
                receiver,
                cancel: Some(cancel),
                phase: Arc::new(AtomicU8::new(1)),
                bytes: CHUNK + 4096,
                resource: Some(stream),
                finishing: false,
            },
        );
        Ok(())
    }
    pub(super) fn finish_upload(&mut self, id: usize, stream: usize) -> Result<(), &'static str> {
        if self.jobs.len() >= MAX_JOBS {
            return Err("HttpConcurrencyLimit");
        }
        let slot = self.uploads.get(&stream).ok_or("HttpUploadClosed")?.clone();
        if slot.busy.swap(true, Ordering::SeqCst) {
            return Err("HttpUploadBusy");
        }
        let (sender, receiver) = mpsc::channel();
        let (cancel, cancelled) = oneshot::channel();
        self.runtime.spawn(async move {
            let future=async {
                let response={let mut data=slot.data.lock().await;data.sender=None;data.response.take()};
                let Some(response)=response else{return failure("HttpUploadClosed","Unknown",0);};
                response.await.unwrap_or_else(|_|failure("HttpWorker","Unknown",0))
            };
            let outcome=tokio::select!{biased;_=cancelled=>failure("HttpCancelled","Unknown",0),v=tokio::time::timeout_at(slot.deadline,future)=>v.unwrap_or_else(|_|failure("HttpTimeout","Unknown",0))};
            slot.busy.store(false,Ordering::SeqCst);let _=sender.send(outcome);
        });
        self.jobs.insert(
            id,
            Job {
                receiver,
                cancel: Some(cancel),
                phase: Arc::new(AtomicU8::new(1)),
                bytes: 4096,
                resource: Some(stream),
                finishing: true,
            },
        );
        Ok(())
    }
    pub(super) fn close_upload(&mut self, stream: usize) {
        if let Some(slot) = self.uploads.remove(&stream) {
            slot.closed.store(true, Ordering::SeqCst);
            if let Some(cancel) = slot.cancel.lock().unwrap().take() {
                let _ = cancel.send(());
            }
            self.retired_uploads.push(slot.clone());
            self.runtime.spawn(async move {
                slot.data.lock().await.sender = None;
            });
        }
    }
}
impl Drop for Upload {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.get_mut().unwrap().take() {
            let _ = cancel.send(());
        }
    }
}
impl crate::Runtime {
    pub fn write_http(&mut self, stream: usize, body: Arc<Vec<u8>>) -> crate::Result<usize> {
        use sha2::{Digest, Sha256};
        self.charge_native_work(body.len().saturating_add(1))?;
        if self
            .sensitive_values
            .iter()
            .any(|s| !s.is_empty() && body.windows(s.len()).any(|w| w == s.as_bytes()))
            || self
                .sensitive_bytes
                .iter()
                .any(|s| body.windows(s.len()).any(|w| w == s.as_slice()))
        {
            return Err(crate::Error::InvalidOperation(
                "SecretObservationUnrecordable: private HTTP upload".into(),
            ));
        }
        let (id, fresh) = self.begin_async_external(
            "http.write.v1",
            &serde_json::to_vec(&(
                stream,
                body.len(),
                format!("{:x}", Sha256::digest(body.as_slice())),
            ))
            .unwrap(),
            4096,
        )?;
        if fresh {
            let submitted = (|| {
                if self.network_host.is_none() {
                    return Err("HttpUploadClosed");
                }
                self.network_host.as_mut().unwrap().admission = CHUNK + 4096;
                let budget = self.enforce_budget();
                self.network_host.as_mut().unwrap().admission = 0;
                if budget.is_err() {
                    return Err("HttpMemoryLimit");
                }
                self.network_host
                    .as_mut()
                    .unwrap()
                    .write_upload(id, stream, body)
            })();
            if let Err(code) = submitted {
                self.finish_async_external(id, Ok(failure(code, "Unknown", 0)))?;
            }
        }
        Ok(id)
    }
    pub fn finish_http(&mut self, stream: usize, response_limit: usize) -> crate::Result<usize> {
        let (id, fresh) = self.begin_async_external(
            "http.finish.v1",
            &(stream as u64).to_le_bytes(),
            response_limit.min(MAX_BODY).div_ceil(3) * 4 + MAX_HEADERS * 2 + 4096,
        )?;
        if fresh {
            let submitted = self
                .network_host
                .as_mut()
                .ok_or("HttpUploadClosed")
                .and_then(|h| h.finish_upload(id, stream));
            if let Err(code) = submitted {
                self.finish_async_external(id, Ok(failure(code, "Unknown", 0)))?;
            }
        }
        Ok(id)
    }
}
