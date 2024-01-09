use bevy::prelude::*;
use std::{marker::PhantomData, ptr, mem};

use openxr as xr;
use xr::{
    sys::{Space, SystemPassthroughProperties2FB},
    CompositionLayerFlags, PassthroughCapabilityFlagsFB, CompositionLayerBase, Graphics,
};

use crate::resources::XrInstance;
#[derive(Resource)]
pub struct XrPassthroughLayer(pub xr::PassthroughLayer);
#[derive(Resource)]
pub struct XrPassthrough(pub xr::Passthrough);
fn cvt(x: xr::sys::Result) -> xr::Result<xr::sys::Result> {
    if x.into_raw() >= 0 {
        Ok(x)
    } else {
        Err(x)
    }
}

#[derive(Copy, Clone)]
#[repr(transparent)]
pub(crate) struct CompositionLayerPassthrough<'a, G: xr::Graphics> {
    inner: xr::sys::CompositionLayerPassthroughFB,
    _marker: PhantomData<&'a G>,
}
impl<'a, G: Graphics> std::ops::Deref for CompositionLayerPassthrough<'a, G> {
    type Target = CompositionLayerBase<'a, G>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { mem::transmute(&self.inner) }
    }
}

impl<'a, G: xr::Graphics> CompositionLayerPassthrough<'a, G> {
    pub(crate) fn from_xr_passthrough_layer(layer: &XrPassthroughLayer) -> Self {
        Self {
            inner: xr::sys::CompositionLayerPassthroughFB {
                ty: xr::sys::CompositionLayerPassthroughFB::TYPE,
                next: ptr::null(),
                flags: CompositionLayerFlags::BLEND_TEXTURE_SOURCE_ALPHA,
                space: Space::NULL,
                layer_handle: *layer.0.inner(),
            },
            _marker: PhantomData,
        }
    }
}
#[inline]
pub fn supports_passthrough(instance: &XrInstance, system: xr::SystemId) -> xr::Result<bool> {
    unsafe {
        let mut hand = xr::sys::SystemPassthroughProperties2FB {
            ty: SystemPassthroughProperties2FB::TYPE,
            next: ptr::null(),
            capabilities: PassthroughCapabilityFlagsFB::PASSTHROUGH_CAPABILITY,
        };
        let mut p = xr::sys::SystemProperties::out(&mut hand as *mut _ as _);
        cvt((instance.fp().get_system_properties)(
            instance.as_raw(),
            system,
            p.as_mut_ptr(),
        ))?;
        Ok(
            (hand.capabilities & PassthroughCapabilityFlagsFB::PASSTHROUGH_CAPABILITY)
                == PassthroughCapabilityFlagsFB::PASSTHROUGH_CAPABILITY,
        )
    }
}
