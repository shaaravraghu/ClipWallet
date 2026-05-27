//! Raw NSPasteboard access for data types arboard doesn't support (macOS).
//! On Linux, RTF / file-URL clipboard formats are unsupported — returns None/false.

use std::path::PathBuf;

// ── macOS implementation via Objective-C ──────────────────────────────────────

#[cfg(target_os = "macos")]
mod macos {
    use objc::rc::autoreleasepool;
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    use std::path::PathBuf;

    const NS_RTF_PBOARD_TYPE:      &str = "public.rtf";
    const NS_FILE_URL_PBOARD_TYPE: &str = "public.file-url";

    pub fn read_rtf() -> Option<Vec<u8>> {
        autoreleasepool(|| unsafe {
            let pb: *mut Object = msg_send![class!(NSPasteboard), generalPasteboard];
            let ns_type = nsstring(NS_RTF_PBOARD_TYPE);
            let data: *mut Object = msg_send![pb, dataForType: ns_type];
            if data.is_null() { return None; }
            let len: usize = msg_send![data, length];
            let ptr: *const u8 = msg_send![data, bytes];
            if ptr.is_null() || len == 0 { return None; }
            Some(std::slice::from_raw_parts(ptr, len).to_vec())
        })
    }

    pub fn write_rtf(bytes: &[u8]) -> bool {
        autoreleasepool(|| unsafe {
            let pb: *mut Object = msg_send![class!(NSPasteboard), generalPasteboard];
            let _: i64 = msg_send![pb, clearContents];
            let ns_type = nsstring(NS_RTF_PBOARD_TYPE);
            let ns_data = nsdata(bytes);
            msg_send![pb, setData: ns_data forType: ns_type]
        })
    }

    pub fn read_file_paths() -> Option<Vec<PathBuf>> {
        autoreleasepool(|| unsafe {
            let pb: *mut Object = msg_send![class!(NSPasteboard), generalPasteboard];
            let ns_type = nsstring(NS_FILE_URL_PBOARD_TYPE);
            let classes: *mut Object = msg_send![class!(NSArray), arrayWithObject: class!(NSURL)];
            let options: *mut Object = msg_send![class!(NSDictionary), dictionary];
            let urls: *mut Object = msg_send![pb, readObjectsForClasses: classes options: options];
            if urls.is_null() { return None; }
            let count: usize = msg_send![urls, count];
            if count == 0 { return None; }
            let mut paths = Vec::with_capacity(count);
            for i in 0..count {
                let url: *mut Object = msg_send![urls, objectAtIndex: i];
                let is_file: bool = msg_send![url, isFileURL];
                if !is_file { continue; }
                let ns_path: *mut Object = msg_send![url, path];
                let cstr: *const i8 = msg_send![ns_path, UTF8String];
                if cstr.is_null() { continue; }
                let s = std::ffi::CStr::from_ptr(cstr).to_string_lossy().to_string();
                paths.push(PathBuf::from(s));
            }
            if paths.is_empty() { None } else { Some(paths) }
        })
    }

    pub fn write_file_paths(paths: &[PathBuf]) -> bool {
        autoreleasepool(|| unsafe {
            let pb: *mut Object = msg_send![class!(NSPasteboard), generalPasteboard];
            let _: i64 = msg_send![pb, clearContents];
            let url_array: *mut Object = msg_send![class!(NSMutableArray), array];
            for path in paths {
                let s = path.to_string_lossy();
                let ns_str = nsstring(&s);
                let url: *mut Object = msg_send![class!(NSURL), fileURLWithPath: ns_str];
                let _: () = msg_send![url_array, addObject: url];
            }
            msg_send![pb, writeObjects: url_array]
        })
    }

    unsafe fn nsstring(s: &str) -> *mut Object {
        let cls = class!(NSString);
        let bytes = s.as_ptr() as *const std::os::raw::c_void;
        let len = s.len();
        let obj: *mut Object = msg_send![cls, alloc];
        let obj: *mut Object = msg_send![obj, initWithBytes: bytes length: len encoding: 4u64];
        let _: () = msg_send![obj, autorelease];
        obj
    }

    unsafe fn nsdata(bytes: &[u8]) -> *mut Object {
        let cls = class!(NSData);
        let ptr = bytes.as_ptr() as *const std::os::raw::c_void;
        msg_send![cls, dataWithBytes: ptr length: bytes.len()]
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

pub fn read_rtf() -> Option<Vec<u8>> {
    #[cfg(target_os = "macos")]
    { macos::read_rtf() }
    #[cfg(not(target_os = "macos"))]
    { None }
}

pub fn write_rtf(_bytes: &[u8]) -> bool {
    #[cfg(target_os = "macos")]
    { macos::write_rtf(_bytes) }
    #[cfg(not(target_os = "macos"))]
    { false }
}

pub fn read_file_paths() -> Option<Vec<PathBuf>> {
    #[cfg(target_os = "macos")]
    { macos::read_file_paths() }
    #[cfg(not(target_os = "macos"))]
    { None }
}

pub fn write_file_paths(_paths: &[PathBuf]) -> bool {
    #[cfg(target_os = "macos")]
    { macos::write_file_paths(_paths) }
    #[cfg(not(target_os = "macos"))]
    { false }
}