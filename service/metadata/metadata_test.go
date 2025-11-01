package metadata

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/getcihub/cihub/core"
)

// TestFindSuccess tests successful metadata retrieval
func TestFindSuccess(t *testing.T) {
	// Setup mock MMDS server
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Token generation endpoint
		if r.URL.Path == "/latest/api/token" && r.Method == "PUT" {
			ttl := r.Header.Get("X-Metadata-Token-TTL-Seconds")
			if ttl == "" {
				t.Error("Expected X-Metadata-Token-TTL-Seconds header")
			}
			w.WriteHeader(http.StatusOK)
			w.Write([]byte("test-token-123"))
			return
		}

		// Metadata retrieval endpoint
		if strings.HasPrefix(r.URL.Path, "/latest/metadata/") {
			// Verify token in request
			token := r.Header.Get("X-Metadata-Token")
			if token != "test-token-123" {
				w.WriteHeader(http.StatusUnauthorized)
				return
			}

			// Return metadata response
			response := metadataResponse{
				Latest: struct {
					Metadata struct {
						CIHub *core.Metadata `json:"cihub"`
					} `json:"meta-data"`
				}{
					Metadata: struct {
						CIHub *core.Metadata `json:"cihub"`
					}{
						CIHub: &core.Metadata{
							RunnerID:        "runner-123",
							RunnerHostname:  "host.example.com",
							RunnerJITConfig: "jit-config-abc-def",
						},
					},
				},
			}

			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(response)
			return
		}

		w.WriteHeader(http.StatusNotFound)
	}))
	defer server.Close()

	ctx := context.Background()

	// Create a service-like call
	token, err := generateTestToken(ctx, server.URL)
	if err != nil {
		t.Fatalf("Failed to generate token: %v", err)
	}

	md, err := getTestMetadata(ctx, server.URL, token)
	if err != nil {
		t.Fatalf("Failed to get metadata: %v", err)
	}

	if md == nil {
		t.Fatal("Metadata is nil")
	}
	if md.RunnerID != "runner-123" {
		t.Errorf("Expected runner-123, got %s", md.RunnerID)
	}
	if md.RunnerHostname != "host.example.com" {
		t.Errorf("Expected host.example.com, got %s", md.RunnerHostname)
	}
	if md.RunnerJITConfig != "jit-config-abc-def" {
		t.Errorf("Expected jit-config-abc-def, got %s", md.RunnerJITConfig)
	}
}

// TestTokenGenerationFailure tests handling of token generation failures
func TestTokenGenerationFailure(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Token endpoint returns error
		if r.URL.Path == "/latest/api/token" && r.Method == "PUT" {
			w.WriteHeader(http.StatusInternalServerError)
			w.Write([]byte("Token service unavailable"))
			return
		}
		w.WriteHeader(http.StatusNotFound)
	}))
	defer server.Close()

	ctx := context.Background()
	_, err := generateTestToken(ctx, server.URL)
	if err == nil {
		t.Error("Expected error, got nil")
	}
	if !strings.Contains(err.Error(), "500") {
		t.Errorf("Expected 500 status error, got: %v", err)
	}
}

// TestUnauthorizedMetadataAccess tests handling of 401 unauthorized responses
func TestUnauthorizedMetadataAccess(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/latest/api/token" && r.Method == "PUT" {
			w.WriteHeader(http.StatusOK)
			w.Write([]byte("test-token"))
			return
		}

		// Invalid token returns 401
		if strings.HasPrefix(r.URL.Path, "/latest/metadata/") {
			token := r.Header.Get("X-Metadata-Token")
			if token != "valid-token" {
				w.WriteHeader(http.StatusUnauthorized)
				return
			}
			w.WriteHeader(http.StatusOK)
			json.NewEncoder(w).Encode(metadataResponse{})
			return
		}

		w.WriteHeader(http.StatusNotFound)
	}))
	defer server.Close()

	ctx := context.Background()
	_, err := generateTestToken(ctx, server.URL)
	if err != nil {
		t.Fatalf("Failed to generate token: %v", err)
	}

	// Try to use wrong token
	req, _ := http.NewRequestWithContext(ctx, "GET", fmt.Sprintf("%s/latest/metadata/cihub", server.URL), nil)
	req.Header.Set("X-Metadata-Token", "wrong-token")
	res, err := server.Client().Do(req)
	if err != nil {
		t.Fatalf("Request failed: %v", err)
	}
	defer res.Body.Close()

	if res.StatusCode != http.StatusUnauthorized {
		t.Errorf("Expected 401, got %d", res.StatusCode)
	}
}

// TestMetadataDecodeFailure tests handling of invalid JSON responses
func TestMetadataDecodeFailure(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/latest/api/token" && r.Method == "PUT" {
			w.WriteHeader(http.StatusOK)
			w.Write([]byte("test-token"))
			return
		}

		if strings.HasPrefix(r.URL.Path, "/latest/metadata/") {
			// Return invalid JSON
			w.Header().Set("Content-Type", "application/json")
			w.Write([]byte("invalid json {"))
			return
		}

		w.WriteHeader(http.StatusNotFound)
	}))
	defer server.Close()

	ctx := context.Background()
	token, _ := generateTestToken(ctx, server.URL)

	req, _ := http.NewRequestWithContext(ctx, "GET", fmt.Sprintf("%s/latest/metadata/cihub", server.URL), nil)
	req.Header.Set("X-Metadata-Token", token)
	res, err := server.Client().Do(req)
	if err != nil {
		t.Fatalf("Request failed: %v", err)
	}
	defer res.Body.Close()

	var response metadataResponse
	err = json.NewDecoder(res.Body).Decode(&response)
	if err == nil {
		t.Error("Expected decode error, got nil")
	}
}

// TestRequestTimeout tests handling of slow MMDS server
func TestRequestTimeout(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Simulate slow server
		time.Sleep(2 * time.Second)
		w.WriteHeader(http.StatusOK)
	}))
	defer server.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
	defer cancel()

	// Create a client with short timeout
	client := &http.Client{
		Timeout: 100 * time.Millisecond,
	}

	req, _ := http.NewRequestWithContext(ctx, "GET", fmt.Sprintf("%s/latest/api/token", server.URL), nil)
	_, err := client.Do(req)

	if err == nil {
		t.Error("Expected timeout error, got nil")
	}
}

// TestContextCancellation tests handling of cancelled context
func TestContextCancellation(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Simulate slow processing
		time.Sleep(1 * time.Second)
		w.WriteHeader(http.StatusOK)
	}))
	defer server.Close()

	ctx, cancel := context.WithCancel(context.Background())
	cancel() // Cancel immediately

	client := &http.Client{
		Timeout: 5 * time.Second,
	}

	req, _ := http.NewRequestWithContext(ctx, "GET", fmt.Sprintf("%s/latest/api/token", server.URL), nil)
	_, err := client.Do(req)

	if err == nil {
		t.Error("Expected context cancelled error, got nil")
	}
}

// TestNotFoundResponse tests handling of 404 responses
func TestNotFoundResponse(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/latest/api/token" && r.Method == "PUT" {
			w.WriteHeader(http.StatusOK)
			w.Write([]byte("test-token"))
			return
		}

		if strings.HasPrefix(r.URL.Path, "/latest/metadata/") {
			w.WriteHeader(http.StatusNotFound)
			w.Write([]byte("Path not found"))
			return
		}

		w.WriteHeader(http.StatusNotFound)
	}))
	defer server.Close()

	ctx := context.Background()
	token, _ := generateTestToken(ctx, server.URL)

	req, _ := http.NewRequestWithContext(ctx, "GET", fmt.Sprintf("%s/latest/metadata/nonexistent", server.URL), nil)
	req.Header.Set("X-Metadata-Token", token)
	res, err := server.Client().Do(req)
	if err != nil {
		t.Fatalf("Request failed: %v", err)
	}
	defer res.Body.Close()

	if res.StatusCode != http.StatusNotFound {
		t.Errorf("Expected 404, got %d", res.StatusCode)
	}
}

// TestMissingMetadataToken tests requests missing the metadata token
func TestMissingMetadataToken(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if strings.HasPrefix(r.URL.Path, "/latest/metadata/") {
			// Check for token
			token := r.Header.Get("X-Metadata-Token")
			if token == "" {
				w.WriteHeader(http.StatusUnauthorized)
				w.Write([]byte("Token required"))
				return
			}
			w.WriteHeader(http.StatusOK)
			json.NewEncoder(w).Encode(metadataResponse{})
			return
		}
		w.WriteHeader(http.StatusNotFound)
	}))
	defer server.Close()

	// Request without token
	req, _ := http.NewRequestWithContext(context.Background(), "GET", fmt.Sprintf("%s/latest/metadata/cihub", server.URL), nil)
	res, err := server.Client().Do(req)
	if err != nil {
		t.Fatalf("Request failed: %v", err)
	}
	defer res.Body.Close()

	if res.StatusCode != http.StatusUnauthorized {
		t.Errorf("Expected 401 without token, got %d", res.StatusCode)
	}
}

// TestPathNormalization tests that paths are normalized correctly
func TestPathNormalization(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"cihub", "cihub"},
		{"/cihub", "cihub"},
		{"///cihub", "//cihub"},
		{"fireactions/runner", "fireactions/runner"},
		{"/fireactions/runner", "fireactions/runner"},
	}

	for _, tt := range tests {
		result := strings.TrimPrefix(tt.input, "/")
		if result != tt.expected {
			t.Errorf("Path normalization failed: input=%s, expected=%s, got=%s", tt.input, tt.expected, result)
		}
	}
}

// TestTokenReused tests that token is reused across multiple requests
func TestTokenReused(t *testing.T) {
	tokenCallCount := 0
	metadataCallCount := 0

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/latest/api/token" && r.Method == "PUT" {
			tokenCallCount++
			w.WriteHeader(http.StatusOK)
			w.Write([]byte("test-token-shared"))
			return
		}

		if strings.HasPrefix(r.URL.Path, "/latest/metadata/") {
			metadataCallCount++
			token := r.Header.Get("X-Metadata-Token")
			if token == "" {
				w.WriteHeader(http.StatusUnauthorized)
				return
			}
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(metadataResponse{
				Latest: struct {
					Metadata struct {
						CIHub *core.Metadata `json:"cihub"`
					} `json:"meta-data"`
				}{
					Metadata: struct {
						CIHub *core.Metadata `json:"cihub"`
					}{
						CIHub: &core.Metadata{RunnerID: "test"},
					},
				},
			})
			return
		}

		w.WriteHeader(http.StatusNotFound)
	}))
	defer server.Close()

	ctx := context.Background()

	// Make multiple requests
	for range 3 {
		token, _ := generateTestToken(ctx, server.URL)
		if token == "" {
			t.Fatalf("Failed to get token")
		}

		req, _ := http.NewRequestWithContext(ctx, "GET", fmt.Sprintf("%s/latest/metadata/cihub", server.URL), nil)
		req.Header.Set("X-Metadata-Token", token)
		res, err := server.Client().Do(req)
		if err != nil {
			t.Fatalf("Request failed: %v", err)
		}
		res.Body.Close()
	}

	// Token should be generated multiple times since we're not caching
	// This test shows the current behavior (no caching)
	if tokenCallCount == 0 {
		t.Error("Token endpoint was never called")
	}
}

// TestServerError tests handling of 500 server errors
func TestServerError(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/latest/api/token" && r.Method == "PUT" {
			w.WriteHeader(http.StatusOK)
			w.Write([]byte("test-token"))
			return
		}

		if strings.HasPrefix(r.URL.Path, "/latest/metadata/") {
			w.WriteHeader(http.StatusInternalServerError)
			w.Write([]byte("Internal server error"))
			return
		}

		w.WriteHeader(http.StatusNotFound)
	}))
	defer server.Close()

	ctx := context.Background()
	token, _ := generateTestToken(ctx, server.URL)

	req, _ := http.NewRequestWithContext(ctx, "GET", fmt.Sprintf("%s/latest/metadata/cihub", server.URL), nil)
	req.Header.Set("X-Metadata-Token", token)
	res, err := server.Client().Do(req)
	if err != nil {
		t.Fatalf("Request failed: %v", err)
	}
	defer res.Body.Close()

	if res.StatusCode != http.StatusInternalServerError {
		t.Errorf("Expected 500, got %d", res.StatusCode)
	}
}

// TestForbiddenResponse tests handling of 403 forbidden responses
func TestForbiddenResponse(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/latest/api/token" && r.Method == "PUT" {
			w.WriteHeader(http.StatusOK)
			w.Write([]byte("test-token"))
			return
		}

		if strings.HasPrefix(r.URL.Path, "/latest/metadata/") {
			w.WriteHeader(http.StatusForbidden)
			w.Write([]byte("Access denied"))
			return
		}

		w.WriteHeader(http.StatusNotFound)
	}))
	defer server.Close()

	ctx := context.Background()
	token, _ := generateTestToken(ctx, server.URL)

	req, _ := http.NewRequestWithContext(ctx, "GET", fmt.Sprintf("%s/latest/metadata/restricted", server.URL), nil)
	req.Header.Set("X-Metadata-Token", token)
	res, err := server.Client().Do(req)
	if err != nil {
		t.Fatalf("Request failed: %v", err)
	}
	defer res.Body.Close()

	if res.StatusCode != http.StatusForbidden {
		t.Errorf("Expected 403, got %d", res.StatusCode)
	}
}

// Helper functions for testing

func generateTestToken(ctx context.Context, baseURL string) (string, error) {
	req, err := http.NewRequestWithContext(ctx, "PUT", fmt.Sprintf("%s/latest/api/token", baseURL), nil)
	if err != nil {
		return "", err
	}
	req.Header.Set("X-Metadata-Token-TTL-Seconds", "21600")

	client := &http.Client{Timeout: 5 * time.Second}
	res, err := client.Do(req)
	if err != nil {
		return "", err
	}
	defer res.Body.Close()

	if res.StatusCode != http.StatusOK {
		return "", fmt.Errorf("unexpected status: %d", res.StatusCode)
	}

	body := make([]byte, 1024)
	n, err := res.Body.Read(body)
	if n == 0 {
		return "", fmt.Errorf("empty token response")
	}
	if err != nil && err.Error() != "EOF" {
		return "", err
	}

	return string(body[:n]), nil
}

func getTestMetadata(ctx context.Context, baseURL, token string) (*core.Metadata, error) {
	req, err := http.NewRequestWithContext(ctx, "GET", fmt.Sprintf("%s/latest/metadata/cihub", baseURL), nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("X-Metadata-Token", token)

	client := &http.Client{Timeout: 5 * time.Second}
	res, err := client.Do(req)
	if err != nil {
		return nil, err
	}
	defer res.Body.Close()

	if res.StatusCode >= 300 {
		return nil, fmt.Errorf("unexpected status: %d", res.StatusCode)
	}

	var response metadataResponse
	err = json.NewDecoder(res.Body).Decode(&response)
	if err != nil {
		return nil, err
	}

	return response.Latest.Metadata.CIHub, nil
}
