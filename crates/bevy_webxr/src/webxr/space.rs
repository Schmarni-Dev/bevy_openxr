use std::sync::mpsc::{channel, Receiver, Sender};

use bevy::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{XrReferenceSpace, XrReferenceSpaceType};

use crate::{
    future_util::ToJsFuture,
    session::{WxrSession, WxrSessionCreated},
};

pub struct WxrReferenceSpacePlugin;

impl Plugin for WxrReferenceSpacePlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = channel();
        app.add_event::<WxrSetReferenceSpace>();
        app.insert_non_send_resource(RefSpaceCreatedReader(rx));
        app.insert_non_send_resource(RefSpaceCreatedWriter(tx));
        app.add_systems(
            Last,
            create_reference_space.run_if(resource_exists::<WxrSession>),
        );
        app.add_systems(
            First,
            insert_reference_space.run_if(resource_exists::<WxrSession>),
        );
        app.add_systems(
            Update,
            request_ref_space.run_if(on_event::<WxrSessionCreated>()),
        );
    }
}

fn request_ref_space(mut event: EventWriter<WxrSetReferenceSpace>) {
    event.send(WxrSetReferenceSpace(WxrReferenceSpaceType::LocalFloor));
}

#[derive(Clone, Copy)]
pub enum WxrReferenceSpaceType {
    Local,
    LocalFloor,
    View,
    Unbounded,
    BoundedFloor,
}
impl From<WxrReferenceSpaceType> for XrReferenceSpaceType {
    fn from(value: WxrReferenceSpaceType) -> Self {
        match value {
            WxrReferenceSpaceType::Local => Self::Local,
            WxrReferenceSpaceType::LocalFloor => Self::LocalFloor,
            WxrReferenceSpaceType::View => Self::Viewer,
            WxrReferenceSpaceType::Unbounded => Self::Unbounded,
            WxrReferenceSpaceType::BoundedFloor => Self::BoundedFloor,
        }
    }
}

#[derive(Event, Clone, Copy)]
pub struct WxrSetReferenceSpace(pub WxrReferenceSpaceType);

fn create_reference_space(
    session: Res<WxrSession>,
    mut event: EventReader<WxrSetReferenceSpace>,
    sender: NonSend<RefSpaceCreatedWriter>,
) {
    for e in event.read() {
        let session = session.session.clone();
        let sender = sender.clone();
        let e = e.0;
        spawn_local(async move {
            let space = session
                .request_reference_space(e.into())
                .to_future()
                .await
                .unwrap()
                .dyn_into::<XrReferenceSpace>()
                .unwrap();
            sender.send(space).unwrap()
        });
    }
}
fn insert_reference_space(mut cmds: Commands, reader: NonSend<RefSpaceCreatedReader>) {
    while let Ok(space) = reader.0.try_recv() {
        cmds.insert_resource(WxrReferenceSpace(space));
    }
}

#[derive(Resource, Deref, DerefMut)]
pub struct WxrReferenceSpace(pub XrReferenceSpace);
// SAFETY: idk probably very bad if somehow used with treads
unsafe impl Send for WxrReferenceSpace {}
unsafe impl Sync for WxrReferenceSpace {}
#[derive(DerefMut, Deref)]
struct RefSpaceCreatedReader(Receiver<XrReferenceSpace>);
#[derive(DerefMut, Deref, Clone)]
struct RefSpaceCreatedWriter(Sender<XrReferenceSpace>);
