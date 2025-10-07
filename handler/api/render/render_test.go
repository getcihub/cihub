package render

import (
	"encoding/json"
	"net/http/httptest"
	"testing"
)

func TestSuccess(t *testing.T) {
	w := httptest.NewRecorder()
	data := map[string]string{"key": "value"}

	Success(w, "test_reason", data, 200)

	if w.Code != 200 {
		t.Errorf("Expected status 200, got %d", w.Code)
	}

	var resp Response
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if resp.Error {
		t.Errorf("Expected error=false, got error=true")
	}

	if resp.Reason != "test_reason" {
		t.Errorf("Expected reason='test_reason', got '%s'", resp.Reason)
	}

	if resp.Data == nil {
		t.Errorf("Expected data to be present")
	}
}

func TestError(t *testing.T) {
	w := httptest.NewRecorder()

	Error(w, "test_error", 400)

	if w.Code != 400 {
		t.Errorf("Expected status 400, got %d", w.Code)
	}

	var resp Response
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if !resp.Error {
		t.Errorf("Expected error=true, got error=false")
	}

	if resp.Reason != "test_error" {
		t.Errorf("Expected reason='test_error', got '%s'", resp.Reason)
	}

	if resp.Data != nil {
		t.Errorf("Expected data to be nil for error responses")
	}
}

func TestOK(t *testing.T) {
	w := httptest.NewRecorder()
	data := map[string]int{"id": 1}

	OK(w, ReasonResolved, data)

	if w.Code != 200 {
		t.Errorf("Expected status 200, got %d", w.Code)
	}

	var resp Response
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if resp.Error {
		t.Errorf("Expected error=false")
	}

	if resp.Reason != ReasonResolved {
		t.Errorf("Expected reason='%s', got '%s'", ReasonResolved, resp.Reason)
	}
}

func TestCreated(t *testing.T) {
	w := httptest.NewRecorder()
	data := map[string]int{"id": 1}

	Created(w, data)

	if w.Code != 201 {
		t.Errorf("Expected status 201, got %d", w.Code)
	}

	var resp Response
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if resp.Reason != ReasonCreated {
		t.Errorf("Expected reason='%s', got '%s'", ReasonCreated, resp.Reason)
	}
}

func TestUnauthorized(t *testing.T) {
	w := httptest.NewRecorder()

	Unauthorized(w)

	if w.Code != 401 {
		t.Errorf("Expected status 401, got %d", w.Code)
	}

	authHeader := w.Header().Get("WWW-Authenticate")
	if authHeader != `Basic realm="CIHub"` {
		t.Errorf("Expected WWW-Authenticate header, got '%s'", authHeader)
	}

	var resp Response
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if !resp.Error {
		t.Errorf("Expected error=true")
	}

	if resp.Reason != ReasonUnauthorized {
		t.Errorf("Expected reason='%s', got '%s'", ReasonUnauthorized, resp.Reason)
	}
}

func TestForbidden(t *testing.T) {
	w := httptest.NewRecorder()

	Forbidden(w)

	if w.Code != 403 {
		t.Errorf("Expected status 403, got %d", w.Code)
	}

	var resp Response
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if resp.Reason != ReasonForbidden {
		t.Errorf("Expected reason='%s', got '%s'", ReasonForbidden, resp.Reason)
	}
}

func TestNotFound(t *testing.T) {
	w := httptest.NewRecorder()

	NotFound(w)

	if w.Code != 404 {
		t.Errorf("Expected status 404, got %d", w.Code)
	}

	var resp Response
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if resp.Reason != ReasonNotFound {
		t.Errorf("Expected reason='%s', got '%s'", ReasonNotFound, resp.Reason)
	}
}

func TestBadRequest(t *testing.T) {
	w := httptest.NewRecorder()

	BadRequest(w)

	if w.Code != 400 {
		t.Errorf("Expected status 400, got %d", w.Code)
	}

	var resp Response
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if resp.Reason != ReasonBadRequest {
		t.Errorf("Expected reason='%s', got '%s'", ReasonBadRequest, resp.Reason)
	}
}

func TestInternalError(t *testing.T) {
	w := httptest.NewRecorder()

	InternalError(w)

	if w.Code != 500 {
		t.Errorf("Expected status 500, got %d", w.Code)
	}

	var resp Response
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if resp.Reason != ReasonInternalError {
		t.Errorf("Expected reason='%s', got '%s'", ReasonInternalError, resp.Reason)
	}
}

func TestNotImplemented(t *testing.T) {
	w := httptest.NewRecorder()

	NotImplemented(w)

	if w.Code != 501 {
		t.Errorf("Expected status 501, got %d", w.Code)
	}

	var resp Response
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if resp.Reason != ReasonNotImplemented {
		t.Errorf("Expected reason='%s', got '%s'", ReasonNotImplemented, resp.Reason)
	}
}

func TestCustomReason(t *testing.T) {
	w := httptest.NewRecorder()
	customReason := "user_authenticated"

	OK(w, customReason, map[string]string{"token": "abc123"})

	var resp Response
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if resp.Reason != customReason {
		t.Errorf("Expected reason='%s', got '%s'", customReason, resp.Reason)
	}
}

func TestUnauthorizedWithReason(t *testing.T) {
	w := httptest.NewRecorder()
	customReason := "invalid_credentials"

	UnauthorizedWithReason(w, customReason)

	if w.Code != 401 {
		t.Errorf("Expected status 401, got %d", w.Code)
	}

	var resp Response
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if resp.Reason != customReason {
		t.Errorf("Expected reason='%s', got '%s'", customReason, resp.Reason)
	}
}

func TestBadRequestWithReason(t *testing.T) {
	w := httptest.NewRecorder()
	customReason := "missing_required_field"

	BadRequestWithReason(w, customReason)

	if w.Code != 400 {
		t.Errorf("Expected status 400, got %d", w.Code)
	}

	var resp Response
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if resp.Reason != customReason {
		t.Errorf("Expected reason='%s', got '%s'", customReason, resp.Reason)
	}
}

func TestContentType(t *testing.T) {
	w := httptest.NewRecorder()

	OK(w, ReasonResolved, nil)

	contentType := w.Header().Get("Content-Type")
	if contentType != "application/json" {
		t.Errorf("Expected Content-Type='application/json', got '%s'", contentType)
	}
}
