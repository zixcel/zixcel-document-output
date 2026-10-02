use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, Stream, dictionary};

pub fn document_bytes() -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages = doc.new_object_id();
    let font =
        doc.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Helvetica"});
    let image = doc.add_object(Stream::new(dictionary! {"Type"=>"XObject", "Subtype"=>"Image", "Width"=>1, "Height"=>1, "ColorSpace"=>"DeviceRGB", "BitsPerComponent"=>8}, vec![0,0,0]));
    let form = doc.add_object(Stream::new(dictionary! {"Type"=>"XObject", "Subtype"=>"Form", "BBox"=>vec![0.into(),0.into(),10.into(),10.into()], "Resources"=>dictionary! {"XObject"=>dictionary! {"Im1"=>image}}}, Content {operations:vec![Operation::new("Do",vec![Object::Name(b"Im1".to_vec())])]}.encode().unwrap()));
    let resources = doc.add_object(dictionary! {"Font"=>dictionary! {"F1"=>font}, "XObject"=>dictionary! {"Im1"=>image,"Fm1"=>form}});
    let mut children = Vec::new();
    for index in 0..4 {
        let mut operations = Vec::new();
        if index == 0 || index == 2 {
            operations.extend([
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
                Operation::new("Td", vec![20.into(), 50.into()]),
                Operation::new(
                    "Tj",
                    vec![Object::string_literal(if index == 0 {
                        "First page"
                    } else {
                        "Third page"
                    })],
                ),
                Operation::new("ET", vec![]),
            ]);
        }
        if index == 1 || index == 2 {
            operations.push(Operation::new(
                "Do",
                vec![Object::Name(if index == 1 {
                    b"Im1".to_vec()
                } else {
                    b"Fm1".to_vec()
                })],
            ));
        }
        let content = doc.add_object(Stream::new(
            dictionary! {},
            Content { operations }.encode().unwrap(),
        ));
        let page = doc.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"Contents"=>content,"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()]});
        children.push(Object::Reference(page));
    }
    // Inherited resources exercise the page-tree lookup rather than only page-local dictionaries.
    doc.objects.insert(
        pages,
        Object::Dictionary(
            dictionary! {"Type"=>"Pages","Kids"=>children,"Count"=>4,"Resources"=>resources},
        ),
    );
    let catalog = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    doc.trailer.set("Root", catalog);
    let mut title = vec![0xfe, 0xff];
    for unit in "例示資料".encode_utf16() {
        title.extend(unit.to_be_bytes());
    }
    let info = doc.add_object(dictionary! {"Title"=>Object::String(title,lopdf::StringFormat::Hexadecimal),"Author"=>Object::string_literal("Example Maintainer"),"CreationDate"=>Object::string_literal("D:20250314051247+09'00'")});
    doc.trailer.set("Info", info);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}
