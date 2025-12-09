use std::collections::HashMap;

pub fn get_alias_map() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    // Game Dev
    m.insert("raylib", "https://github.com/raysan5/raylib.git");
    m.insert("sdl2", "https://github.com/libsdl-org/SDL.git");
    m.insert("sfml", "https://github.com/SFML/SFML.git");
    
    // Utilities
    m.insert("json", "https://github.com/nlohmann/json.git");
    m.insert("nlohmann_json", "https://github.com/nlohmann/json.git");
    m.insert("fmt", "https://github.com/fmtlib/fmt.git");
    m.insert("spdlog", "https://github.com/gabime/spdlog.git");
    
    // Web / Networking
    m.insert("mongoose", "https://github.com/cesanta/mongoose.git");
    m.insert("httplib", "https://github.com/yhirose/cpp-httplib.git");
    m.insert("cpp-httplib", "https://github.com/yhirose/cpp-httplib.git");
    m.insert("cpr", "https://github.com/libcpr/cpr.git"); // requests like

    // Testing
    m.insert("catch2", "https://github.com/catchorg/Catch2.git");
    m.insert("doctest", "https://github.com/doctest/doctest.git");
    m.insert("gtest", "https://github.com/google/googletest.git");

    m
}

pub fn resolve_alias(name: &str) -> Option<String> {
    let map = get_alias_map();
    map.get(name).map(|s| s.to_string())
}
