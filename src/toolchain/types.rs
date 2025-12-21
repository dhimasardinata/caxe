use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Supported compiler types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::upper_case_acronyms)]
pub enum CompilerType {
    /// Microsoft Visual C++ (cl.exe)
    MSVC,
    /// Clang with MSVC compatibility (clang-cl.exe)
    ClangCL,
    /// Clang/LLVM (clang++.exe or clang.exe)
    Clang,
    /// GNU Compiler Collection (g++.exe or gcc.exe)
    GCC,
}

#[allow(dead_code)]
impl CompilerType {
    pub fn is_msvc_compatible(&self) -> bool {
        matches!(self, CompilerType::MSVC | CompilerType::ClangCL)
    }

    pub fn uses_msvc_flags(&self) -> bool {
        matches!(self, CompilerType::MSVC | CompilerType::ClangCL)
    }
}

/// Represents a discovered compiler toolchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Toolchain {
    /// Type of compiler
    pub compiler_type: CompilerType,

    /// Absolute path to the C compiler
    pub cc_path: PathBuf,

    /// Absolute path to the C++ compiler
    pub cxx_path: PathBuf,

    /// Absolute path to the linker (may be same as compiler for GCC/Clang)
    pub linker_path: PathBuf,

    /// Compiler version string
    pub version: String,

    /// MSVC toolset version (e.g., "14.38.33130")
    pub msvc_toolset_version: Option<String>,

    /// Windows SDK version (e.g., "10.0.22621.0")
    pub windows_sdk_version: Option<String>,

    /// Visual Studio installation path
    pub vs_install_path: Option<PathBuf>,

    /// Environment variables needed for this toolchain (PATH, INCLUDE, LIB, LIBPATH)
    pub env_vars: HashMap<String, String>,
}

#[allow(dead_code)]
impl Toolchain {
    /// Creates a new toolchain with minimal info (for non-MSVC compilers)
    pub fn new_simple(compiler_type: CompilerType, cxx_path: PathBuf, version: String) -> Self {
        let cc_path = if cxx_path.to_string_lossy().contains("++") {
            PathBuf::from(cxx_path.to_string_lossy().replace("++", ""))
        } else {
            cxx_path.clone()
        };

        Self {
            compiler_type,
            cc_path,
            cxx_path: cxx_path.clone(),
            linker_path: cxx_path,
            version,
            msvc_toolset_version: None,
            windows_sdk_version: None,
            vs_install_path: None,
            env_vars: HashMap::new(),
        }
    }

    /// Get the appropriate compiler for C++ files
    pub fn get_cxx_compiler(&self) -> &PathBuf {
        &self.cxx_path
    }

    /// Get the appropriate compiler for C files
    pub fn get_cc_compiler(&self) -> &PathBuf {
        &self.cc_path
    }

    /// Check if this toolchain requires special environment setup
    pub fn needs_env_setup(&self) -> bool {
        !self.env_vars.is_empty()
    }

    /// Get fingerprint for cache invalidation
    pub fn fingerprint(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.cxx_path.hash(&mut hasher);
        self.version.hash(&mut hasher);
        if let Some(ref v) = self.msvc_toolset_version {
            v.hash(&mut hasher);
        }
        if let Some(ref v) = self.windows_sdk_version {
            v.hash(&mut hasher);
        }
        format!("{:x}", hasher.finish())
    }
}

/// Visual Studio installation info from vswhere
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct VSInstallation {
    pub install_path: PathBuf,
    pub display_name: String,
    pub version: String,
    pub product_id: String, // e.g., "Microsoft.VisualStudio.Product.BuildTools"
}

/// Error type for toolchain operations
#[derive(Debug)]
pub enum ToolchainError {
    /// No suitable toolchain found
    NotFound(String),
    #[cfg(windows)]
    /// Error running vswhere
    VsWhereError(String),
    #[cfg(windows)]
    /// Error loading vcvars environment
    VcVarsError(String),
    /// IO error
    IoError(std::io::Error),
}

impl std::fmt::Display for ToolchainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolchainError::NotFound(msg) => write!(f, "Toolchain not found: {}", msg),
            #[cfg(windows)]
            ToolchainError::VsWhereError(msg) => write!(f, "vswhere error: {}", msg),
            #[cfg(windows)]
            ToolchainError::VcVarsError(msg) => write!(f, "vcvars error: {}", msg),
            ToolchainError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for ToolchainError {}

impl From<std::io::Error> for ToolchainError {
    fn from(e: std::io::Error) -> Self {
        ToolchainError::IoError(e)
    }
}
