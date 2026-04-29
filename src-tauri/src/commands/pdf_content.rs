//! Shared helpers for manipulating PDF page resources and content streams.
//!
//! These functions extract the repeated "Phase 1/2/3/4" resource-and-content
//! management pattern that was copy-pasted across functions like
//! `add_text_watermark`, `sign_pdf`, `add_text_to_page`, `add_rectangle`,
//! `add_highlight`, and `add_whiteout` in `pdf_ops.rs`.

use lopdf::{Document, Object, ObjectId};

use crate::error::{AppError, AppResult};

/// Get or create the Resources dictionary for a page, always returning a
/// reference to a standalone object.
///
/// Handles three cases:
/// - **No Resources**: creates an empty dictionary object.
/// - **Resources is a reference**: returns the existing reference as-is.
/// - **Resources is an inline dictionary**: promotes it to a standalone object.
///
/// Returns `(needs_page_update, resources_object_id)`. When
/// `needs_page_update` is `true` the caller must set the page's `Resources`
/// entry to `Object::Reference(resources_object_id)`.
pub fn get_or_create_resources(
    doc: &mut Document,
    page_id: &ObjectId,
) -> AppResult<(bool, ObjectId)> {
    let page = doc
        .get_object(*page_id)
        .map_err(|e| AppError::Pdf(format!("Page object error: {}", e)))?;
    let page_dict = page
        .as_dict()
        .map_err(|e| AppError::Pdf(format!("Page dict error: {}", e)))?;

    match page_dict.get(b"Resources") {
        Err(_) => {
            let res_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::new()));
            Ok((true, res_id))
        }
        Ok(res_obj) => {
            if let Ok(r) = res_obj.as_reference() {
                Ok((false, r))
            } else {
                // Inline dictionary -- promote to standalone object
                let res_id = doc.add_object(res_obj.clone());
                Ok((true, res_id))
            }
        }
    }
}

/// Ensure a sub-dictionary entry exists inside a resources dictionary and set
/// (or merge) a key-value pair into it.
///
/// `category` is the resources sub-dictionary name, e.g. `"XObject"`,
/// `"Font"`, or `"ExtGState"`. `key` is the entry name within that
/// sub-dictionary (e.g. `b"F1"` or `b"GS1"`). `value` is the object to
/// insert -- typically an `Object::Reference(..)`.
///
/// If the category sub-dictionary does not yet exist it is created.
/// Existing entries in the category dictionary are preserved.
pub fn ensure_resource_entry(
    doc: &mut Document,
    res_id: ObjectId,
    category: &str,
    key: &[u8],
    value: Object,
) -> AppResult<()> {
    let res_obj = doc
        .objects
        .get_mut(&res_id)
        .ok_or_else(|| AppError::Pdf("Resources object not found".into()))?;
    let res_dict = res_obj
        .as_dict_mut()
        .map_err(|e| AppError::Pdf(format!("Resources dict error: {}", e)))?;

    // Create the category sub-dictionary if missing
    if res_dict.get(category.as_bytes()).is_err() {
        res_dict.set(category, Object::Dictionary(lopdf::Dictionary::new()));
    }

    // Set the key inside the category sub-dictionary
    if let Ok(cat_obj) = res_dict.get_mut(category.as_bytes()) {
        if let Ok(cat_dict) = cat_obj.as_dict_mut() {
            cat_dict.set(key, value);
        }
    }

    Ok(())
}

/// Append a new content stream to a page's existing Contents.
///
/// Handles three cases:
/// - **No Contents**: sets Contents to a reference to `content_id`.
/// - **Contents is a single reference**: creates an array `[old_ref, new_ref]`.
/// - **Contents is an array**: appends the new reference to the array.
///
/// An inline Contents array is promoted to a standalone object first so that
/// the existing content is never lost.
pub fn append_content_stream(
    doc: &mut Document,
    page_id: &ObjectId,
    content_id: ObjectId,
) -> AppResult<()> {
    // Phase 1: read current Contents without holding a mutable borrow
    let has_contents_ref: Option<ObjectId> = {
        let page_obj = doc
            .objects
            .get(page_id)
            .ok_or_else(|| AppError::Pdf("Page object not found".into()))?;
        let dict = page_obj
            .as_dict()
            .map_err(|e| AppError::Pdf(format!("Page dict error: {}", e)))?;
        match dict.get(b"Contents") {
            Ok(c) => {
                if let Ok(r) = c.as_reference() {
                    Some(r)
                } else if let Ok(arr) = c.as_array() {
                    // Inline array -- promote to standalone object
                    Some(doc.add_object(Object::Array(arr.clone())))
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    };

    // Phase 2: build the new Contents value
    let new_contents_ref = match has_contents_ref {
        Some(existing_ref) => {
            let arr = Object::Array(vec![
                Object::Reference(existing_ref),
                Object::Reference(content_id),
            ]);
            doc.add_object(arr)
        }
        None => content_id,
    };

    // Phase 3: write back to the page
    let page_obj = doc
        .objects
        .get_mut(page_id)
        .ok_or_else(|| AppError::Pdf("Page object not found".into()))?;
    let dict = page_obj
        .as_dict_mut()
        .map_err(|e| AppError::Pdf(format!("Page dict error: {}", e)))?;
    dict.set("Contents", Object::Reference(new_contents_ref));

    Ok(())
}

/// Get a mutable reference to the page dictionary for the given `page_id`.
///
/// This is a thin safe wrapper around the repeated
/// `doc.get_object(*page_id).unwrap().as_dict().unwrap()` pattern.
pub fn get_page_dict_mut<'a>(
    doc: &'a mut Document,
    page_id: &ObjectId,
) -> AppResult<&'a mut lopdf::Dictionary> {
    let page_obj = doc
        .objects
        .get_mut(page_id)
        .ok_or_else(|| AppError::Pdf("Page object not found".into()))?;
    page_obj
        .as_dict_mut()
        .map_err(|e| AppError::Pdf(format!("Page dict error: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a minimal single-page PDF with an empty page object (no Resources, no Contents).
    fn create_empty_page_doc() -> (ObjectId, Document) {
        let mut doc = Document::with_version("1.4");
        let catalog_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::new()));
        let pages_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"Pages".to_vec())),
            (b"Count".to_vec(), Object::Integer(1)),
            (b"Kids".to_vec(), Object::Array(vec![])),
        ])));
        if let Some(cat) = doc.objects.get_mut(&catalog_id) {
            if let Ok(d) = cat.as_dict_mut() {
                d.set("Type", Object::Name(b"Catalog".to_vec()));
                d.set("Pages", Object::Reference(pages_id));
            }
        }
        let page_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"Page".to_vec())),
            (b"Parent".to_vec(), Object::Reference(pages_id)),
            (b"MediaBox".to_vec(), Object::Array(vec![
                Object::Integer(0), Object::Integer(0),
                Object::Integer(612), Object::Integer(792),
            ])),
        ])));
        if let Some(pages_obj) = doc.objects.get_mut(&pages_id) {
            if let Ok(d) = pages_obj.as_dict_mut() {
                d.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
            }
        }
        doc.trailer.set(b"Root", Object::Reference(catalog_id));
        (page_id, doc)
    }

    /// Create a page that already has a Resources reference and a Contents stream.
    fn create_page_with_resources_and_content() -> (ObjectId, Document) {
        let (page_id, mut doc) = create_empty_page_doc();
        let res_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"Font".to_vec(), Object::Dictionary(lopdf::Dictionary::new())),
        ])));
        let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            b"BT /F1 12 Tf (Hello) Tj ET".to_vec(),
        )));
        if let Some(page_obj) = doc.objects.get_mut(&page_id) {
            if let Ok(dict) = page_obj.as_dict_mut() {
                dict.set("Resources", Object::Reference(res_id));
                dict.set("Contents", Object::Reference(content_id));
            }
        }
        (page_id, doc)
    }

    #[test]
    fn test_get_or_create_resources_no_resources() {
        let (page_id, mut doc) = create_empty_page_doc();
        let (needs_update, res_id) = get_or_create_resources(&mut doc, &page_id).unwrap();
        assert!(needs_update);
        // The returned res_id should point to an empty dictionary
        let obj = doc.get_object(res_id).unwrap();
        assert!(obj.as_dict().is_ok());
    }

    #[test]
    fn test_get_or_create_resources_existing_reference() {
        let (page_id, mut doc) = create_page_with_resources_and_content();
        let (needs_update, res_id) = get_or_create_resources(&mut doc, &page_id).unwrap();
        assert!(!needs_update);
        // res_id should point to a dict with a Font entry
        let obj = doc.get_object(res_id).unwrap();
        let dict = obj.as_dict().unwrap();
        assert!(dict.get(b"Font").is_ok());
    }

    #[test]
    fn test_get_or_create_resources_inline_promotion() {
        let (page_id, mut doc) = create_empty_page_doc();
        // Manually set an inline Resources dictionary
        {
            let page_obj = doc.objects.get_mut(&page_id).unwrap();
            let dict = page_obj.as_dict_mut().unwrap();
            dict.set("Resources", Object::Dictionary(lopdf::Dictionary::from_iter(vec![
                (b"XObject".to_vec(), Object::Dictionary(lopdf::Dictionary::new())),
            ])));
        }
        let (needs_update, res_id) = get_or_create_resources(&mut doc, &page_id).unwrap();
        assert!(needs_update);
        // The promoted object should retain the XObject entry
        let obj = doc.get_object(res_id).unwrap();
        let dict = obj.as_dict().unwrap();
        assert!(dict.get(b"XObject").is_ok());
    }

    #[test]
    fn test_ensure_resource_entry_new_category() {
        let (page_id, mut doc) = create_empty_page_doc();
        let (needs_update, res_id) = get_or_create_resources(&mut doc, &page_id).unwrap();
        assert!(needs_update);

        let font_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::new()));
        ensure_resource_entry(
            &mut doc, res_id, "Font", b"F1", Object::Reference(font_id),
        ).unwrap();

        let obj = doc.get_object(res_id).unwrap();
        let res_dict = obj.as_dict().unwrap();
        let font_dict = res_dict.get(b"Font").unwrap().as_dict().unwrap();
        assert!(font_dict.get(b"F1").is_ok());
    }

    #[test]
    fn test_ensure_resource_entry_preserves_existing() {
        let (page_id, mut doc) = create_page_with_resources_and_content();
        let (_, res_id) = get_or_create_resources(&mut doc, &page_id).unwrap();

        let gs_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::new()));
        ensure_resource_entry(
            &mut doc, res_id, "ExtGState", b"GS1", Object::Reference(gs_id),
        ).unwrap();

        let obj = doc.get_object(res_id).unwrap();
        let res_dict = obj.as_dict().unwrap();
        // Font should still exist
        assert!(res_dict.get(b"Font").is_ok());
        // ExtGState should now exist
        assert!(res_dict.get(b"ExtGState").is_ok());
    }

    #[test]
    fn test_append_content_stream_no_existing() {
        let (page_id, mut doc) = create_empty_page_doc();
        let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            b"q Q".to_vec(),
        )));
        append_content_stream(&mut doc, &page_id, content_id).unwrap();

        let page_obj = doc.objects.get(&page_id).unwrap();
        let dict = page_obj.as_dict().unwrap();
        let contents = dict.get(b"Contents").unwrap();
        // Should be a direct reference to content_id
        assert_eq!(contents.as_reference().unwrap(), content_id);
    }

    #[test]
    fn test_append_content_stream_with_existing_ref() {
        let (page_id, mut doc) = create_page_with_resources_and_content();
        // page already has Contents pointing to a stream
        let new_content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            b"q Q".to_vec(),
        )));
        append_content_stream(&mut doc, &page_id, new_content_id).unwrap();

        let page_obj = doc.objects.get(&page_id).unwrap();
        let dict = page_obj.as_dict().unwrap();
        let contents_ref = dict.get(b"Contents").unwrap().as_reference().unwrap();
        // Contents should now point to an array object
        let contents_obj = doc.get_object(contents_ref).unwrap();
        let arr = contents_obj.as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn test_get_page_dict_mut() {
        let (page_id, mut doc) = create_empty_page_doc();
        let dict = get_page_dict_mut(&mut doc, &page_id).unwrap();
        assert!(dict.get(b"MediaBox").is_ok());
    }
}
