# Contributing to USDA

Thank you for your interest in contributing to USDA! This document provides guidelines and instructions for contributing to the project.

## Development Setup

1. **Prerequisites**
   - Rust toolchain (2021 edition)
   - PostgreSQL database
   - SP1 ZK VM framework

2. **Environment Setup**
   ```bash
   # Clone the repository
   git clone https://github.com/yourusername/usda.git
   cd usda

   # Install dependencies
   cargo build

   # Set up database
   createdb usda_test
   export DATABASE_URL=postgres://localhost/usda_test

   # Run database migrations
   psql $DATABASE_URL -f schema.sql
   ```

3. **Running Tests**
   ```bash
   # Run all tests
   cargo test

   # Run specific package tests
   cargo test -p usda-common
   cargo test -p usda-core
   ```

## Code Style

- Follow Rust standard formatting (use `cargo fmt`)
- Run clippy before committing (`cargo clippy`)
- Write descriptive commit messages
- Add tests for new functionality
- Update documentation as needed

## Pull Request Process

1. Create a new branch for your changes
2. Make your changes and commit them with clear messages
3. Run tests and ensure they pass
4. Update documentation if necessary
5. Submit a pull request with a description of your changes

## Commit Message Guidelines

Format: `<type>(<scope>): <subject>`

Types:
- feat: New feature
- fix: Bug fix
- docs: Documentation only changes
- style: Changes that do not affect the meaning of the code
- refactor: Code change that neither fixes a bug nor adds a feature
- test: Adding missing tests
- chore: Changes to the build process or auxiliary tools

Example:
```
feat(transaction): add signature verification
```

## Code Review Process

1. All submissions require review
2. Changes must have tests
3. PR must pass CI checks
4. Documentation must be updated

## License

By contributing, you agree that your contributions will be licensed under the project's MIT License.
