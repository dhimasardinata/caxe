pub struct ProjectTemplate {
    pub toml: String,
    pub code: String,
}

pub fn get(template: &str, name: &str, lang: &str) -> ProjectTemplate {
    match template {
        "raylib" => ProjectTemplate {
            toml: format!(
                r#"[package]
name = "{}"
version = "0.1.0"
edition = "c++17"

[build]
libs = ["gdi32", "user32", "shell32", "winmm", "opengl32"]

[dependencies]
raylib = {{ git = "https://github.com/raysan5/raylib.git", build = "mingw32-make -C src PLATFORM=PLATFORM_DESKTOP", output = "src/libraylib.a" }}
"#,
                name
            ),
            code: r#"#include "raylib.h"
int main() {
    InitWindow(800, 600, "cx + raylib");
    SetTargetFPS(60);
    while (!WindowShouldClose()) {
        BeginDrawing();
        ClearBackground(RAYWHITE);
        DrawText("Hello Raylib!", 190, 200, 20, LIGHTGRAY);
        EndDrawing();
    }
    CloseWindow();
    return 0;
}
"#
            .to_string(),
        },
        "web" => get_web_template(name, lang),
        _ => get_console_template(name, lang),
    }
}

fn get_web_template(name: &str, lang: &str) -> ProjectTemplate {
    if lang == "c" {
        ProjectTemplate {
            toml: format!(
                r#"[package]
name = "{}"
version = "0.1.0"
edition = "c17"

[build]
libs = ["ws2_32"]

[dependencies]
mongoose = {{ git = "https://github.com/cesanta/mongoose.git", build = "clang -c mongoose.c -o libmongoose.a", output = "libmongoose.a" }}
"#,
                name
            ),
            code: r#"#include "mongoose.h"

static void fn(struct mg_connection* c, int ev, void* ev_data) {
  if (ev == MG_EV_HTTP_MSG) {
    mg_http_reply(c, 200, "", "<h1>Hello from C (Mongoose)!</h1>\n");
  }
}

int main() {
  struct mg_mgr mgr;
  mg_mgr_init(&mgr);
  printf("Server running at http://localhost:8000\n");
  mg_http_listen(&mgr, "http://0.0.0.0:8000", fn, NULL);
  for (;;) mg_mgr_poll(&mgr, 1000);
  mg_mgr_free(&mgr);
  return 0;
}
"#
            .to_string(),
        }
    } else {
        ProjectTemplate {
            toml: format!(
                r#"[package]
name = "{}"
version = "0.1.0"
edition = "c++17"

[build]
cflags = ["-D_WIN32_WINNT=0x0A00"]
libs = ["ws2_32"]

[dependencies]
httplib = "https://github.com/yhirose/cpp-httplib.git"
"#,
                name
            ),
            code: r#"#include <iostream>
#include "httplib.h"
int main() {
    httplib::Server svr;
    svr.Get("/", [](const httplib::Request&, httplib::Response& res) {
        res.set_content("<h1>Hello from cx (C++)!</h1>", "text/html");
    });
    std::cout << "Server at http://localhost:8080" << std::endl;
    svr.listen("0.0.0.0", 8080);
    return 0;
}
"#
            .to_string(),
        }
    }
}

fn get_console_template(name: &str, lang: &str) -> ProjectTemplate {
    let dep = if lang == "cpp" {
        "\n[dependencies]\n# json = \"...\""
    } else {
        ""
    };
    let cfg = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "{}"
{}
"#,
        name,
        if lang == "c" { "c17" } else { "c++20" },
        dep
    );

    let code = if lang == "c" {
        "#include <stdio.h>\nint main() { printf(\"Hello cx!\\n\"); return 0; }"
    } else {
        "#include <iostream>\nint main() { std::cout << \"Hello cx!\" << std::endl; return 0; }"
    };

    ProjectTemplate {
        toml: cfg,
        code: code.to_string(),
    }
}
