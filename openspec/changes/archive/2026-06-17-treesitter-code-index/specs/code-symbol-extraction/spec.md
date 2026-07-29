## ADDED Requirements

### Requirement: Extract symbols from TypeScript files
The system SHALL parse TypeScript files using tree-sitter and extract function signatures, method definitions, interface declarations, type aliases, and class declarations.

#### Scenario: TypeScript file with functions and interfaces
- **WHEN** a `.ts` file contains function declarations and interfaces
- **THEN** the system produces a summary with each symbol's name, signature, and JSDoc comment

### Requirement: Extract symbols from Rust files
The system SHALL parse Rust files using tree-sitter and extract function items, impl blocks, structs, enums, and traits.

#### Scenario: Rust file with structs and functions
- **WHEN** a `.rs` file contains struct definitions and function items
- **THEN** the system produces a summary with each symbol's name, signature, and doc comments

### Requirement: Extract symbols from Python files
The system SHALL parse Python files using tree-sitter and extract function and class definitions with docstrings.

#### Scenario: Python file with classes
- **WHEN** a `.py` file contains class and function definitions
- **THEN** the system produces a summary with each symbol's name, signature, and docstring

### Requirement: Skip files with no declarations
The system SHALL skip files that contain no extractable symbol declarations.

#### Scenario: Config file with no symbols
- **WHEN** a file contains only assignments or configuration (no functions/types)
- **THEN** no summary file is produced for it

### Requirement: Produce one summary file per source file
The system SHALL write one markdown summary per source file to `.history/repos/<repo>/<relative-path>.md` containing only extracted symbols.

#### Scenario: Summary file structure
- **WHEN** symbols are extracted from a source file
- **THEN** the summary contains frontmatter (source, repo, file, date) and sections for functions and types with signatures and docstrings

### Requirement: Summaries are small and embeddable
The system SHALL produce summaries that are under 4KB per file on average by including only names, signatures, and first-line docstrings.

#### Scenario: Large file with many functions
- **WHEN** a file has 50+ functions
- **THEN** the summary includes all signatures but truncates docstrings to first sentence

### Requirement: Include git blame metadata per symbol
The system SHALL run git blame on each symbol's line range and include the most recent author, modification date, commit hash, and associated merge request in the summary.

#### Scenario: Symbol with blame info
- **WHEN** a function is extracted from a git-tracked file
- **THEN** the summary includes the author name, last modified date, short commit hash, and MR title/number (from commit message)

#### Scenario: Commit message contains MR reference
- **WHEN** a commit message contains "Merge branch" or "See merge request !NNN"
- **THEN** the MR number and title are extracted and included

#### Scenario: File not in a git repo
- **WHEN** a file is not tracked by git
- **THEN** the summary omits author/date/commit/MR fields without error
