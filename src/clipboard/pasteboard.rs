//! Raw NSPasteboard access for data types arboard doesn't support.
//! Covers: RTF, file URLs (as pointer paths).

use std::path::PathBuf;

// ── Objective-C imports ───────────────────────────────────────────────────────
use objc::rc::autoreleasepool;
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};

// NSPasteboard type strings
const NS_RTF_PBOARD_TYPE:        &str = "public.rtf";
const NS_FILE_URL_PBOARD_TYPE:   &str = "public.file-url";
const NS_STRING_PBOARD_TYPE:     &str = "public.utf8-plain-text";

/// Read RTF bytes from the system clipboard if present.
/// Returns None if no RTF data is available.
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

/// Extract plain text from RTF bytes using NSAttributedString.
pub fn rtf_to_plain_text(bytes: &[u8]) -> Option<String> {
    autoreleasepool(|| unsafe {
        let data = nsdata(bytes);
        let attr_str: *mut Object = msg_send![class!(NSAttributedString), alloc];
        let attr_str: *mut Object = msg_send![
            attr_str,
            initWithRTF: data
            documentAttributes: std::ptr::null_mut::<Object>()
        ];
        if attr_str.is_null() { return None; }
        let _: *mut Object = msg_send![attr_str, autorelease];
        let ns_str: *mut Object = msg_send![attr_str, string];
        if ns_str.is_null() { return None; }
        let cstr: *const i8 = msg_send![ns_str, UTF8String];
        if cstr.is_null() { return None; }
        Some(std::ffi::CStr::from_ptr(cstr).to_string_lossy().to_string())
    })
}

/// Write RTF bytes to the system clipboard.
pub fn write_rtf(bytes: &[u8]) -> bool {
    autoreleasepool(|| unsafe {
        let pb: *mut Object = msg_send![class!(NSPasteboard), generalPasteboard];
        let _: i64 = msg_send![pb, clearContents];

        let ns_type = nsstring(NS_RTF_PBOARD_TYPE);
        let ns_data = nsdata(bytes);

        let result: bool = msg_send![pb, setData: ns_data forType: ns_type];
        result
    })
}

/// Read file URLs from the clipboard as PathBufs (pointer-style).
/// Never reads file contents — just the paths.
pub fn read_file_paths() -> Option<Vec<PathBuf>> {
    autoreleasepool(|| unsafe {
        let pb: *mut Object = msg_send![class!(NSPasteboard), generalPasteboard];
        let ns_type = nsstring(NS_FILE_URL_PBOARD_TYPE);

        // Try reading as file URL list first
        let classes: *mut Object = msg_send![
            class!(NSArray),
            arrayWithObject: class!(NSURL)
        ];
        let options: *mut Object = msg_send![
            class!(NSDictionary),
            dictionary
        ];
        let urls: *mut Object = msg_send![
            pb,
            readObjectsForClasses: classes
            options: options
        ];

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

            let s = std::ffi::CStr::from_ptr(cstr)
                .to_string_lossy()
                .to_string();
            paths.push(PathBuf::from(s));
        }

        if paths.is_empty() { None } else { Some(paths) }
    })
}

/// Write file paths to the clipboard as file URLs.
pub fn write_file_paths(paths: &[PathBuf]) -> bool {
    autoreleasepool(|| unsafe {
        let pb: *mut Object = msg_send![class!(NSPasteboard), generalPasteboard];
        let _: i64 = msg_send![pb, clearContents];

        let url_array: *mut Object = msg_send![class!(NSMutableArray), array];
        for path in paths {
            let s = path.to_string_lossy();
            let ns_str = nsstring(&s);
            let url: *mut Object = msg_send![
                class!(NSURL),
                fileURLWithPath: ns_str
            ];
            let _: () = msg_send![url_array, addObject: url];
        }

        let result: bool = msg_send![pb, writeObjects: url_array];
        result
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Create an NSString from a Rust &str
unsafe fn nsstring(s: &str) -> *mut Object {
    let cls = class!(NSString);
    let bytes = s.as_ptr() as *const std::os::raw::c_void;
    let len   = s.len();
    let obj: *mut Object = msg_send![cls, alloc];
    let obj: *mut Object = msg_send![obj, initWithBytes: bytes
                           length: len
                         encoding: 4u64]; // NSUTF8StringEncoding = 4
    msg_send![obj, autorelease]
}

/// Create an NSData from a byte slice
unsafe fn nsdata(bytes: &[u8]) -> *mut Object {
    let cls = class!(NSData);
    let ptr = bytes.as_ptr() as *const std::os::raw::c_void;
    msg_send![cls, dataWithBytes: ptr length: bytes.len()]
}