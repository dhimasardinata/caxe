//! Project templates for `cx new`.
//!
//! This module provides starter templates for different project types.
//!
//! ## Available Templates
//!
//! - `console` - Basic console application (default)
//! - `sdl2` - SDL2 window application
//! - `sdl3` - SDL3 modern API application
//! - `opengl` - OpenGL with GLFW and GLAD
//! - `raylib` - Raylib game framework
//! - `web` - HTTP server (cpp-httplib or mongoose)
//! - `arduino` - Arduino/IoT sketch

pub fn get_template(name: &str, lang: &str, template: &str) -> (String, String) {
    match template {
        "sdl2" => (
            format!(
                r#"[package]
name = "{}"
version = "0.1.0"
edition = "c++17"

[build]
libs = ["user32", "gdi32", "shell32", "winmm", "imm32", "ole32", "oleaut32", "version", "uuid", "advapi32", "setupapi", "dinput8"]
flags = ["/Dmain=SDL_main"]
subsystem = "windows"

[dependencies]
SDL2 = {{ git = "https://github.com/libsdl-org/SDL.git", branch = "SDL2", build = "cmake -S . -B build -DCMAKE_BUILD_TYPE=Release -DSDL_TEST=OFF && cmake --build build --config Release", output = "build/Release/SDL2.lib, build/Release/SDL2main.lib" }}
"#,
                name
            ),
            r#"#include <SDL.h>
#include <iostream>

int main(int argc, char* argv[]) {
    if (SDL_Init(SDL_INIT_VIDEO) < 0) {
        std::cerr << "SDL could not initialize! SDL_Error: " << SDL_GetError() << std::endl;
        return 1;
    }

    SDL_Window* window = SDL_CreateWindow(
        "SDL2 Window (caxe)",
        SDL_WINDOWPOS_UNDEFINED, SDL_WINDOWPOS_UNDEFINED,
        800, 600,
        SDL_WINDOW_SHOWN
    );

    if (window == nullptr) {
        std::cerr << "Window could not be created! SDL_Error: " << SDL_GetError() << std::endl;
        return 1;
    }

    SDL_Surface* screenSurface = SDL_GetWindowSurface(window);
    SDL_FillRect(screenSurface, nullptr, SDL_MapRGB(screenSurface->format, 0xFF, 0xFF, 0xFF));
    SDL_UpdateWindowSurface(window);

    SDL_Delay(2000);
    SDL_DestroyWindow(window);
    SDL_Quit();
    return 0;
}
"#
            .to_string(),
        ),
        "opengl" => (
            format!(
                r#"[package]
name = "{}"
version = "0.1.0"
edition = "c++17"

[build]
libs = ["user32", "gdi32", "shell32", "opengl32", "glfw3"]
flags = ["/MD"]

[dependencies]
glfw = {{ git = "https://github.com/glfw/glfw.git", tag = "3.3.9", build = "cmake -S . -B build -DGLFW_BUILD_EXAMPLES=OFF -DGLFW_BUILD_TESTS=OFF -DGLFW_BUILD_DOCS=OFF && cmake --build build --config Release", output = "build/src/Release/glfw3.lib" }}
glad = {{ git = "https://github.com/Dav1dde/glad.git", branch = "glad2", build = "pip install --user jinja2 && python -m glad --api gl:core=3.3 --out-path dist c", output = "dist/src/gl.c" }}
"#,
                name
            ),
            r#"#include <glad/gl.h>
#include <GLFW/glfw3.h>
#include <iostream>

void framebuffer_size_callback(GLFWwindow* window, int width, int height) {
    glViewport(0, 0, width, height);
}

int main() {
    if (!glfwInit()) {
        std::cerr << "Failed to initialize GLFW" << std::endl;
        return -1;
    }

    GLFWwindow* window = glfwCreateWindow(800, 600, "OpenGL (caxe)", NULL, NULL);
    if (window == NULL) {
        std::cerr << "Failed to create window" << std::endl;
        glfwTerminate();
        return -1;
    }
    glfwMakeContextCurrent(window);
    glfwSetFramebufferSizeCallback(window, framebuffer_size_callback);

    if (!gladLoadGL((GLADloadfunc)glfwGetProcAddress)) {
        std::cerr << "Failed to initialize GLAD" << std::endl;
        return -1;
    }

    while (!glfwWindowShouldClose(window)) {
        glClearColor(0.2f, 0.3f, 0.3f, 1.0f);
        glClear(GL_COLOR_BUFFER_BIT);

        glfwSwapBuffers(window);
        glfwPollEvents();
    }

    glfwTerminate();
    return 0;
}
"#
            .to_string(),
        ),
        "raylib" => (
            format!(
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
            r#"#include "raylib.h"
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
        ),
        "web" => {
            if lang == "c" {
                (
                    format!(
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
                    r#"#include "mongoose.h"

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
                )
            } else {
                (
                    format!(
                        r#"[package]
name = "{}"
version = "0.1.0"
edition = "c++17"

[build]
flags = ["-D_WIN32_WINNT=0x0A00"]
libs = ["ws2_32"]

[dependencies]
httplib = "https://github.com/yhirose/cpp-httplib.git"
"#,
                        name
                    ),
                    r#"#include <iostream>
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
                )
            }
        }
        "arduino" => (
            format!(
                r#"[package]
name = "{}"
version = "0.1.0"
edition = "c++17"

[arduino]
board = "arduino:avr:uno"
# port = "COM3"  # Uncomment and set your port
"#,
                name
            ),
            format!(
                r#"// {} - Arduino Sketch
// Build: cx build --arduino
// Upload: cx upload -p COM3

void setup() {{
    Serial.begin(9600);
    pinMode(LED_BUILTIN, OUTPUT);
    Serial.println("Hello from {}!");
}}

void loop() {{
    digitalWrite(LED_BUILTIN, HIGH);
    delay(1000);
    digitalWrite(LED_BUILTIN, LOW);
    delay(1000);
}}
"#,
                name, name
            ),
        ),
        "sdl3" => (
            format!(
                r#"[package]
name = "{}"
version = "0.1.0"
edition = "c++17"

[build]
libs = ["user32", "gdi32", "shell32", "winmm", "imm32", "ole32", "oleaut32", "version", "uuid", "advapi32", "setupapi"]
flags = ["/MD"]

[dependencies]
SDL3 = {{ git = "https://github.com/libsdl-org/SDL.git", tag = "release-3.2.4", build = "cmake -S . -B build -DCMAKE_BUILD_TYPE=Release -DSDL_TEST=OFF && cmake --build build --config Release", output = "build/Release/SDL3.lib" }}
"#,
                name
            ),
            r#"#define SDL_MAIN_USE_CALLBACKS 1
#include <SDL3/SDL.h>
#include <SDL3/SDL_main.h>

static SDL_Window* window = nullptr;
static SDL_Renderer* renderer = nullptr;

SDL_AppResult SDL_AppInit(void** appstate, int argc, char* argv[]) {
    if (!SDL_Init(SDL_INIT_VIDEO)) {
        SDL_Log("Couldn't initialize SDL: %s", SDL_GetError());
        return SDL_APP_FAILURE;
    }

    if (!SDL_CreateWindowAndRenderer("SDL3 Window (caxe)", 800, 600, 0, &window, &renderer)) {
        SDL_Log("Couldn't create window/renderer: %s", SDL_GetError());
        return SDL_APP_FAILURE;
    }

    return SDL_APP_CONTINUE;
}

SDL_AppResult SDL_AppEvent(void* appstate, SDL_Event* event) {
    if (event->type == SDL_EVENT_QUIT) {
        return SDL_APP_SUCCESS;
    }
    return SDL_APP_CONTINUE;
}

SDL_AppResult SDL_AppIterate(void* appstate) {
    SDL_SetRenderDrawColor(renderer, 40, 60, 80, 255);
    SDL_RenderClear(renderer);
    SDL_RenderPresent(renderer);
    return SDL_APP_CONTINUE;
}

void SDL_AppQuit(void* appstate, SDL_AppResult result) {
    // Cleanup handled automatically by SDL
}
"#
            .to_string(),
        ),
        _ => {
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
                if lang == "c" { "c23" } else { "c++23" },
                dep
            );

            let code = if lang == "c" {
                "#include <stdio.h>\nint main() { printf(\"Hello cx!\\n\"); return 0; }"
            } else {
                "#include <iostream>\nint main() { std::cout << \"Hello cx!\" << std::endl; return 0; }"
            };
            (cfg, code.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_cpp_template() {
        let (config, code) = get_template("myapp", "cpp", "console");
        assert!(config.contains("name = \"myapp\""));
        assert!(config.contains("c++23"));
        assert!(code.contains("#include <iostream>"));
    }

    #[test]
    fn test_default_c_template() {
        let (config, code) = get_template("myapp", "c", "console");
        assert!(config.contains("name = \"myapp\""));
        assert!(config.contains("c23"));
        assert!(code.contains("#include <stdio.h>"));
    }

    #[test]
    fn test_sdl2_template() {
        let (config, code) = get_template("game", "cpp", "sdl2");
        assert!(config.contains("[dependencies]"));
        assert!(config.contains("SDL2"));
        assert!(code.contains("SDL_Init"));
    }

    #[test]
    fn test_sdl3_template() {
        let (config, code) = get_template("game", "cpp", "sdl3");
        assert!(config.contains("SDL3"));
        assert!(code.contains("SDL_AppInit"));
    }

    #[test]
    fn test_opengl_template() {
        let (config, code) = get_template("render", "cpp", "opengl");
        assert!(config.contains("glfw"));
        assert!(config.contains("glad"));
        assert!(code.contains("gladLoadGL"));
    }

    #[test]
    fn test_raylib_template() {
        let (config, code) = get_template("game", "cpp", "raylib");
        assert!(config.contains("raylib"));
        assert!(code.contains("InitWindow"));
    }

    #[test]
    fn test_arduino_template() {
        let (config, code) = get_template("blink", "cpp", "arduino");
        assert!(config.contains("[arduino]"));
        assert!(config.contains("arduino:avr:uno"));
        assert!(code.contains("void setup()"));
        assert!(code.contains("void loop()"));
    }

    #[test]
    fn test_web_cpp_template() {
        let (config, code) = get_template("server", "cpp", "web");
        assert!(config.contains("httplib"));
        assert!(code.contains("httplib::Server"));
    }

    #[test]
    fn test_web_c_template() {
        let (config, code) = get_template("server", "c", "web");
        assert!(config.contains("mongoose"));
        assert!(code.contains("mg_http_listen"));
    }
}
