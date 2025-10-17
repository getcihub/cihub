# =============================================================================
# 🔄 Core Configuration
# =============================================================================

# Use bash with strict error checking
set shell := ["bash", "-uc"]

# Allow passing arguments to recipes
set positional-arguments

# Common command aliases for convenience
alias c := check
alias t := test
alias r := run
alias help := default

# =============================================================================
# Default Recipe
# =============================================================================

# Show available recipes with their descriptions
@default:
	just --list

# =============================================================================
# Development
# =============================================================================

# Generate code
generate:
	go generate ./...

# Run the specified app
run app:
	go run ./cmd/{{ app }}

# =============================================================================
# Testing & Quality
# =============================================================================

# Run tests
test:
	go test -v -cover ./...

# Format code
fmt:
	go fmt ./...

# Run linter
lint:
	golangci-lint run --fix

# Run go vet
vet:
	go vet ./...

# Run mod tidy
tidy:
	go mod tidy

# Run all checks
check: fmt lint vet tidy test
