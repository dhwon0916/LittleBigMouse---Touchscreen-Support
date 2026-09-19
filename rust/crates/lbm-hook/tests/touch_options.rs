use littlebigmouse_hook::zones::ZonesLayout;

#[test]
fn touch_separation_is_opt_in_on_the_wire() {
    for (attribute, expected) in [
        ("", false),
        ("TouchMouseIndependent=\"False\"", false),
        ("TouchMouseIndependent=\"True\"", true),
    ] {
        let layout = ZonesLayout::from_xml(&format!("<ZonesLayout {attribute}/>")).unwrap();
        assert_eq!(layout.touch_mouse_independent, expected);
    }
}
