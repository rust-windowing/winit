#![allow(unused_imports)]
use super::*;
use wasm_bindgen::prelude::*;
use web_sys::*;
#[wasm_bindgen]
extern "C" {
    # [ wasm_bindgen ( extends = :: js_sys :: Object , js_name = ResizeObservation , typescript_type = "ResizeObservation" ) ]
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[doc = "The `ResizeObservation` class."]
    #[doc = ""]
    #[doc = "[MDN Documentation](https://developer.mozilla.org/en-US/docs/Web/API/ResizeObservation)"]
    #[doc = ""]
    #[doc = "*This API requires the following crate features to be activated: `ResizeObservation`*"]
    pub type ResizeObservation;
    // #[cfg(feature = "Element")]
    # [ wasm_bindgen ( structural , method , getter , js_class = "ResizeObservation" , js_name = target ) ]
    #[doc = "Getter for the `target` field of this object."]
    #[doc = ""]
    #[doc = "[MDN Documentation](https://developer.mozilla.org/en-US/docs/Web/API/ResizeObservation/target)"]
    #[doc = ""]
    #[doc = "*This API requires the following crate features to be activated: `Element`, `ResizeObservation`*"]
    pub fn target(this: &ResizeObservation) -> Element;
    // #[cfg(feature = "ResizeObserverBoxOptions")]
    # [ wasm_bindgen ( structural , method , getter , js_class = "ResizeObservation" , js_name = observedBox ) ]
    #[doc = "Getter for the `observedBox` field of this object."]
    #[doc = ""]
    #[doc = "[MDN Documentation](https://developer.mozilla.org/en-US/docs/Web/API/ResizeObservation/observedBox)"]
    #[doc = ""]
    #[doc = "*This API requires the following crate features to be activated: `ResizeObservation`, `ResizeObserverBoxOptions`*"]
    pub fn observed_box(this: &ResizeObservation) -> ResizeObserverBoxOptions;
    # [ wasm_bindgen ( structural , method , getter , js_class = "ResizeObservation" , js_name = lastReportedSizes ) ]
    #[doc = "Getter for the `lastReportedSizes` field of this object."]
    #[doc = ""]
    #[doc = "[MDN Documentation](https://developer.mozilla.org/en-US/docs/Web/API/ResizeObservation/lastReportedSizes)"]
    #[doc = ""]
    #[doc = "*This API requires the following crate features to be activated: `ResizeObservation`*"]
    pub fn last_reported_sizes(this: &ResizeObservation) -> ::js_sys::Array;
}
