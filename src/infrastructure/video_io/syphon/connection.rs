use super::*;
use std::time::{Duration, Instant};
pub(super) struct Client {
    client: Option<Retained<AnyObject>>,
    name: String,
    local: LocalSources,
    device: Retained<AnyObject>,
    next_probe: Instant,
    deadline: Instant,
    valid_once: bool,
    seen: bool,
}
impl Client {
    pub fn new(name: String, local: LocalSources, device: Retained<AnyObject>) -> PortResult<Self> {
        if name.is_empty() {
            return Err("Choose a Syphon source".into());
        }
        let now = Instant::now();
        Ok(Self {
            client: None,
            name,
            local,
            device,
            next_probe: now,
            deadline: now,
            valid_once: false,
            seen: false,
        })
    }
    fn connect(&self) -> PortResult<Option<Retained<AnyObject>>> {
        unsafe {
            let local = self.local.lock().ok().and_then(|v| {
                v.iter()
                    .find(|(n, _, _)| n == &self.name)
                    .map(|(_, _, v)| v.clone())
            });
            let description = if let Some(local) = local {
                let data = NSData::with_bytes(&local);
                let value: Option<Retained<AnyObject>> = msg_send![class(c"NSPropertyListSerialization")?,propertyListWithData:&*data,options:0usize,format:std::ptr::null_mut::<usize>(),error:std::ptr::null_mut::<*mut AnyObject>()];
                value
            } else {
                servers()?
                    .into_iter()
                    .find(|(n, _)| n == &self.name)
                    .map(|(_, description)| description)
            };
            let Some(description) = description else {
                return Ok(None);
            };
            let allocated: objc2::rc::Allocated<AnyObject> =
                msg_send![class(c"SyphonMetalClient")?, alloc];
            let client: Option<Retained<AnyObject>> = msg_send![allocated,initWithServerDescription:&*description,device:&*self.device,options:std::ptr::null::<AnyObject>(),newFrameHandler:None::<&block2::Block<dyn Fn(*mut AnyObject)>>];
            Ok(client)
        }
    }
    pub fn status(&self) -> Option<&'static str> {
        if self.client.is_none() {
            Some("Waiting for Syphon source")
        } else if !self.seen {
            Some("Waiting for Syphon frame")
        } else {
            None
        }
    }
    pub fn frame(&mut self) -> PortResult<Option<Retained<AnyObject>>> {
        let now = Instant::now();
        if self.client.is_none() && now >= self.next_probe {
            self.next_probe = now + Duration::from_millis(500);
            self.client = self.connect()?;
            self.deadline = now + Duration::from_secs(3);
        }
        let Some(client) = &self.client else {
            return Ok(None);
        };
        unsafe {
            let valid: bool = msg_send![&**client, isValid];
            if valid {
                self.valid_once = true;
            } else if self.valid_once || now >= self.deadline {
                let _: () = msg_send![&**client, stop];
                self.client = None;
                self.valid_once = false;
                self.seen = false;
                return Ok(None);
            }
            let fresh: bool = msg_send![&**client, hasNewFrame];
            if !fresh {
                return Ok(None);
            }
            let image: Option<Retained<AnyObject>> = msg_send![&**client, newFrameImage];
            self.seen |= image.is_some();
            Ok(image)
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        if let Some(client) = &self.client {
            unsafe {
                let _: () = msg_send![&**client, stop];
            }
        }
    }
}
