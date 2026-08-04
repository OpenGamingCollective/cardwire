use std::ffi::{c_char, c_int, c_void};

use khronos_egl::{Instance, Static};

// For legacy device, use egl EXT to check if it's discrete or not
pub fn is_discrete_egl(render: u32) -> Result<bool, String> {
    // Unsafe is required, khronos_egl doesnt include EGL EXT
    // reference:
    // - <https://registry.khronos.org/EGL/extensions/EXT/EGL_EXT_device_enumeration.txt>
    // - <https://registry.khronos.org/EGL/extensions/EXT/EGL_EXT_device_query.txt>
    // - <https://registry.khronos.org/EGL/extensions/EXT/EGL_EXT_device_drm.txt>
    const EGL_DRM_RENDER_NODE_FILE_EXT: c_int = 0x3377;
    const EGL_DEVICE_TYPE_EXT: c_int = 0x3590;
    const EGL_DEVICE_TYPE_DISCRETE_GPU_EXT: isize = 0x3593;

    type EGLDeviceEXT = *mut c_void;

    type EglQueryDeviceExt = unsafe extern "C" fn(c_int, *mut EGLDeviceEXT, *mut c_int) -> c_int;
    type EglQueryDeviceStringExt = unsafe extern "C" fn(EGLDeviceEXT, c_int) -> *const c_char;
    type QueryDeviceAttribExt = unsafe extern "C" fn(EGLDeviceEXT, c_int, *mut isize) -> c_int;

    let render_path = format!("/dev/dri/renderD{}", render);

    let egl = Instance::new(Static);
    let query_devices: EglQueryDeviceExt = unsafe {
        std::mem::transmute(
            egl.get_proc_address("eglQueryDevicesEXT")
                .ok_or("Missing eglQueryDevicesEXT")?,
        )
    };

    let query_device_str: EglQueryDeviceStringExt = unsafe {
        std::mem::transmute(
            egl.get_proc_address("eglQueryDeviceStringEXT")
                .ok_or("Missing eglQueryDeviceStringEXT")?,
        )
    };
    let query_attrib_fn: QueryDeviceAttribExt = unsafe {
        std::mem::transmute(
            egl.get_proc_address("eglQueryDeviceAttribEXT")
                .ok_or("Missing eglQueryDeviceAttribEXT")?,
        )
    };

    unsafe {
        let mut num_devices = 0;
        // eglQueryDevicesEXT returns false on error, so 0
        if query_devices(0, std::ptr::null_mut(), &mut num_devices) == 0 {
            return Err("Error at query_device!!!".to_string());
        }

        let mut device_array = vec![std::ptr::null_mut(); num_devices as usize];

        if query_devices(num_devices, device_array.as_mut_ptr(), &mut num_devices) == 0 {
            return Err("Error at second query_device!!!".to_string());
        }

        for dev_ptr in device_array {
            let c_str_ptr = query_device_str(dev_ptr, EGL_DRM_RENDER_NODE_FILE_EXT);
            if c_str_ptr.is_null() {
                continue;
            }

            if let Ok(drm_path) = std::ffi::CStr::from_ptr(c_str_ptr).to_str()
                && drm_path == render_path
            {
                let mut device_type = 0;
                let ret = query_attrib_fn(dev_ptr, EGL_DEVICE_TYPE_EXT, &mut device_type);

                if ret == 1 {
                    return Ok(device_type == EGL_DEVICE_TYPE_DISCRETE_GPU_EXT);
                } else {
                    // success == 0 (e.g. Mesa AMD drivers missing the extension)
                    return Err(
                        "EGL driver found the device but doesn't support querying GPU type".into(),
                    );
                }
            }
        }
    }
    Ok(false)
}
