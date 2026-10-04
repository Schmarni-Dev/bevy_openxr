use bevy::prelude::{Deref, DerefMut, Resource};
use openxr::ExtensionSet;

#[derive(Clone, Debug, Eq, PartialEq, Deref, DerefMut, Resource)]
pub struct OxrEnabledExtensions(pub OxrExtensions);

#[derive(Clone, Debug, Eq, PartialEq, Deref, DerefMut)]
pub struct OxrExtensions(ExtensionSet);
impl OxrExtensions {
    pub fn raw_mut(&mut self) -> &mut ExtensionSet {
        &mut self.0
    }
    pub fn raw(&self) -> &ExtensionSet {
        &self.0
    }
    pub fn enable_fb_passthrough(&mut self) -> &mut Self {
        self.0.fb_passthrough = true;
        self
    }
    pub fn disable_fb_passthrough(&mut self) -> &mut Self {
        self.0.fb_passthrough = false;
        self
    }
    pub fn enable_hand_tracking(&mut self) -> &mut Self {
        self.0.ext_hand_tracking = true;
        self
    }
    pub fn disable_hand_tracking(&mut self) -> &mut Self {
        self.0.ext_hand_tracking = false;
        self
    }
    pub fn enable_extx_overlay(&mut self) -> &mut Self {
        self.0.extx_overlay = true;
        self
    }
    /// returns true if all of the extensions enabled are also available in `available_exts`
    pub fn is_available(&self, available_exts: &OxrExtensions) -> bool {
        self.clone() & available_exts.clone() == *self
    }
}
impl From<ExtensionSet> for OxrExtensions {
    fn from(value: ExtensionSet) -> Self {
        Self(value)
    }
}
impl From<OxrExtensions> for ExtensionSet {
    fn from(val: OxrExtensions) -> Self {
        val.0
    }
}
impl Default for OxrExtensions {
    fn default() -> Self {
        let exts = ExtensionSet::default();
        //exts.ext_hand_tracking = true;
        Self(exts)
    }
}

macro_rules! unavailable_exts {
    (
        $exts:ty;
        $(
            $(
                #[$meta:meta]
            )*
            $ident:ident
        ),*
        $(,)?
    ) => {
        impl $exts {
            /// Returns any extensions needed by `required_exts` that aren't available in `self`
            pub fn unavailable_exts(&self, required_exts: &Self) -> Vec<std::borrow::Cow<'static, str>> {
                let mut exts = vec![];
                $(
                    $(
                        #[$meta]
                    )*
                    if required_exts.0.$ident && !self.0.$ident {
                        exts.push(std::borrow::Cow::Borrowed(stringify!($ident)))
                    }
                )*
                for ext in required_exts.0.other.iter() {
                    if !self.0.other.contains(ext) {
                        exts.push(std::borrow::Cow::Owned(std::ffi::CStr::from_bytes_with_nul(ext).unwrap().to_str().unwrap().to_string()))
                    }
                }
                exts
            }
        }
    };
}

macro_rules! bitor {
    (
        $exts:ty;
        $(
            $(
                #[$meta:meta]
            )*
            $ident:ident
        ),*
        $(,)?
    ) => {
        impl std::ops::BitOr for $exts {
            type Output = Self;

            fn bitor(self, rhs: Self) -> Self::Output {
                let mut out = ExtensionSet::default();
                $(
                    $(
                        #[$meta]
                    )*
                    {
                        out.$ident = self.0.$ident || rhs.0.$ident;
                    }

                )*
                out.other = self.0.other;
                for ext in rhs.0.other {
                    if !out.other.contains(&ext) {
                        out.other.push(ext);
                    }
                }
                Self(out)
            }
        }
    };
}

macro_rules! bitand {
    (
        $exts:ty;
        $(
            $(
                #[$meta:meta]
            )*
            $ident:ident
        ),*
        $(,)?
    ) => {
        impl std::ops::BitAnd for $exts {
            type Output = Self;

            fn bitand(self, rhs: Self) -> Self::Output {
                let mut out = ExtensionSet::default();
                $(
                    $(
                        #[$meta]
                    )*
                    {
                        out.$ident = self.0.$ident && rhs.0.$ident;
                    }

                )*
                for ext in self.0.other {
                    if rhs.0.other.contains(&ext) {
                        out.other.push(ext);
                    }
                }
                Self(out)
            }
        }
    };
}

macro_rules! impl_ext {
    (
        $(
            $macro:ident
        ),*

    ) => {
        $(
            $macro! {
                OxrExtensions;
                almalence_digital_lens_control,
                #[cfg(target_os = "android")]
                android_trackables,
                #[cfg(target_os = "android")]
                android_device_anchor_persistence,
                #[cfg(target_os = "android")]
                android_face_tracking,
                #[cfg(target_os = "android")]
                android_passthrough_camera_state,
                #[cfg(target_os = "android")]
                android_composition_layer_passthrough_mesh,
                #[cfg(target_os = "android")]
                android_raycast,
                #[cfg(target_os = "android")]
                android_performance_metrics,
                #[cfg(target_os = "android")]
                android_trackables_object,
                #[cfg(target_os = "android")]
                android_unbounded_reference_space,
                #[cfg(target_os = "android")]
                android_anchor_sharing_export,
                #[cfg(target_os = "android")]
                android_mouse_interaction,
                #[cfg(target_os = "android")]
                android_trackables_marker,
                #[cfg(target_os = "android")]
                android_trackables_qr_code,
                #[cfg(target_os = "android")]
                android_trackables_image,
                #[cfg(target_os = "android")]
                android_scene_meshing,
                #[cfg(target_os = "android")]
                android_spatial_object_tracking,
                #[cfg(target_os = "android")]
                android_spatial_discovery_raycast,
                #[cfg(target_os = "android")]
                android_spatial_entity_bound_anchor,
                #[cfg(target_os = "android")]
                android_spatial_component_subsumed_by,
                #[cfg(target_os = "android")]
                android_spatial_anchor_space,
                bd_controller_interaction,
                bd_body_tracking,
                bd_facial_simulation,
                bd_spatial_sensing,
                bd_spatial_anchor,
                bd_spatial_anchor_sharing,
                bd_spatial_scene,
                bd_spatial_mesh,
                bd_future_progress,
                bd_spatial_plane,
                bd_ultra_controller_interaction,
                bd_spatial_audio_rendering,
                epic_view_configuration_fov,
                ext_performance_settings,
                ext_thermal_query,
                ext_debug_utils,
                ext_eye_gaze_interaction,
                ext_view_configuration_depth_range,
                ext_conformance_automation,
                ext_hand_tracking,
                #[cfg(windows)]
                ext_win32_appcontainer_compatible,
                ext_dpad_binding,
                ext_hand_joints_motion_range,
                ext_samsung_odyssey_controller,
                ext_hp_mixed_reality_controller,
                ext_palm_pose,
                ext_frame_synthesis,
                ext_uuid,
                ext_render_model,
                ext_interaction_render_model,
                ext_hand_interaction,
                ext_active_action_set_priority,
                ext_local_floor,
                ext_hand_tracking_data_source,
                ext_plane_detection,
                ext_future,
                ext_user_presence,
                ext_composition_layer_inverted_alpha,
                ext_spatial_entity,
                ext_spatial_plane_tracking,
                ext_spatial_marker_tracking,
                ext_spatial_anchor,
                ext_spatial_persistence,
                ext_spatial_persistence_operations,
                ext_interaction_profile_battery_state_display,
                ext_loader_init_properties,
                ext_view_configuration_views_change,
                fb_composition_layer_image_layout,
                fb_composition_layer_alpha_blend,
                #[cfg(target_os = "android")]
                fb_android_surface_swapchain_create,
                fb_swapchain_update_state,
                fb_composition_layer_secure_content,
                fb_body_tracking,
                fb_display_refresh_rate,
                fb_color_space,
                fb_hand_tracking_mesh,
                fb_hand_tracking_aim,
                fb_hand_tracking_capsules,
                fb_spatial_entity,
                fb_foveation,
                fb_foveation_configuration,
                fb_keyboard_tracking,
                fb_triangle_mesh,
                fb_passthrough,
                fb_render_model,
                fb_spatial_entity_query,
                fb_spatial_entity_storage,
                fb_foveation_vulkan,
                #[cfg(target_os = "android")]
                fb_swapchain_update_state_android_surface,
                fb_swapchain_update_state_opengl_es,
                fb_swapchain_update_state_vulkan,
                fb_touch_controller_pro,
                fb_spatial_entity_sharing,
                fb_space_warp,
                fb_haptic_amplitude_envelope,
                fb_scene,
                fb_scene_capture,
                fb_spatial_entity_container,
                fb_face_tracking,
                fb_eye_tracking_social,
                fb_passthrough_keyboard_hands,
                fb_composition_layer_settings,
                fb_touch_controller_proximity,
                fb_haptic_pcm,
                fb_composition_layer_depth_test,
                fb_spatial_entity_storage_batch,
                fb_spatial_entity_user,
                fb_face_tracking2,
                htc_vive_cosmos_controller_interaction,
                htc_facial_tracking,
                htc_vive_focus3_controller_interaction,
                htc_hand_interaction,
                htc_vive_wrist_tracker_interaction,
                htc_passthrough,
                htc_foveation,
                htc_anchor,
                htc_body_tracking,
                huawei_controller_interaction,
                #[cfg(target_os = "android")]
                khr_android_thread_settings,
                #[cfg(target_os = "android")]
                khr_android_surface_swapchain,
                khr_composition_layer_cube,
                #[cfg(target_os = "android")]
                khr_android_create_instance,
                khr_composition_layer_depth,
                khr_vulkan_swapchain_format_list,
                khr_composition_layer_cylinder,
                khr_composition_layer_equirect,
                khr_opengl_enable,
                khr_opengl_es_enable,
                khr_vulkan_enable,
                #[cfg(windows)]
                khr_d3d11_enable,
                #[cfg(windows)]
                khr_d3d12_enable,
                #[cfg(target_vendor = "apple")]
                khr_metal_enable,
                khr_visibility_mask,
                khr_composition_layer_color_scale_bias,
                #[cfg(windows)]
                khr_win32_convert_performance_counter_time,
                khr_convert_timespec_time,
                khr_loader_init,
                #[cfg(target_os = "android")]
                khr_loader_init_android,
                khr_vulkan_enable2,
                khr_composition_layer_equirect2,
                khr_binding_modification,
                khr_extended_struct_name_lengths,
                khr_swapchain_usage_input_attachment_bit,
                khr_locate_spaces,
                khr_maintenance1,
                khr_generic_controller,
                logitech_mx_ink_stylus_interaction,
                meta_foveation_eye_tracked,
                meta_local_dimming,
                meta_passthrough_preferences,
                meta_virtual_keyboard,
                meta_vulkan_swapchain_create_info,
                meta_performance_metrics,
                meta_detached_controllers,
                meta_headset_id,
                meta_spatial_entity_discovery,
                meta_hand_tracking_microgestures,
                meta_recommended_layer_resolution,
                meta_spatial_entity_persistence,
                meta_passthrough_color_lut,
                meta_spatial_entity_mesh,
                meta_automatic_layer_filter,
                meta_body_tracking_full_body,
                meta_touch_controller_plus,
                meta_passthrough_layer_resumed_event,
                meta_body_tracking_calibration,
                meta_body_tracking_fidelity,
                meta_spatial_entity_sharing,
                meta_environment_depth,
                meta_simultaneous_hands_and_controllers,
                meta_spatial_entity_semantic_label,
                meta_colocation_discovery,
                meta_spatial_entity_group_sharing,
                meta_environment_raycast,
                meta_tile_properties_hint,
                ml_ml2_controller_interaction,
                ml_frame_end_info,
                ml_global_dimmer,
                ml_compat,
                ml_marker_understanding,
                ml_localization_map,
                ml_spatial_anchors,
                ml_spatial_anchors_storage,
                ml_user_calibration,
                ml_system_notifications,
                ml_world_mesh_detection,
                ml_facial_expression,
                ml_view_configuration_depth_range_change,
                mnd_headless,
                mnd_swapchain_usage_input_attachment_bit,
                msft_unbounded_reference_space,
                msft_spatial_anchor,
                msft_spatial_graph_bridge,
                msft_hand_interaction,
                msft_hand_tracking_mesh,
                msft_secondary_view_configuration,
                msft_first_person_observer,
                msft_controller_model,
                #[cfg(windows)]
                msft_perception_anchor_interop,
                #[cfg(windows)]
                msft_holographic_window_attachment,
                msft_composition_layer_reprojection,
                msft_spatial_anchor_persistence,
                #[cfg(target_os = "android")]
                oculus_android_session_state_enable,
                oculus_audio_device_guid,
                oculus_external_camera,
                oppo_controller_interaction,
                qcom_tracking_optimization_settings,
                qcom_hand_tracking_gesture,
                ultraleap_hand_tracking_forearm,
                valve_analog_threshold,
                varjo_quad_views,
                varjo_foveated_rendering,
                varjo_composition_layer_depth_test,
                varjo_environment_depth_estimation,
                varjo_marker_tracking,
                varjo_view_offset,
                varjo_xr4_controller_interaction,
                yvr_controller_interaction,
                extx_overlay,
                mndx_egl_enable,
                mndx_force_feedback_curl,
                htcx_vive_tracker_interaction,
            }
        )*

    };
}

impl_ext!(bitor, bitand, unavailable_exts);
