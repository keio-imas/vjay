use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::NSView;
use objc2_foundation::{NSPoint, NSRect};
// Native GPU surfaces must not steal selection/drag/cable events from GPUI.
define_class!(
    #[unsafe(super(NSView))]
    #[name="VjayPassThroughPreview"]
    #[thread_kind=MainThreadOnly]
    pub(super) struct PreviewView;
    impl PreviewView {
        #[unsafe(method(hitTest:))]
        fn hit_test(&self,_point:NSPoint)->*mut NSView {std::ptr::null_mut()}
    }
);
pub(super) fn create(mtm: MainThreadMarker, frame: NSRect) -> Retained<NSView> {
    let view: Retained<PreviewView> =
        unsafe { msg_send![PreviewView::alloc(mtm),initWithFrame:frame] };
    Retained::into_super(view)
}
