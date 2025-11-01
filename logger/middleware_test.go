package logger

import (
	"net/http/httptest"
	"testing"
)

func TestMiddleware(t *testing.T) {
	t.Skip()
}

func TestMiddleware_GenerateRequestID(t *testing.T) {
	t.Skip()
}

func TestAuthType(t *testing.T) {
	cookieRequest := httptest.NewRequest("GET", "/", nil)
	if authType(cookieRequest) != "cookie" {
		t.Error("auth-type is not cookie")
	}

	bearerRequest := httptest.NewRequest("GET", "/", nil)
	bearerRequest.Header.Add("Authorization", "Bearer test")
	if authType(bearerRequest) != "bearer" {
		t.Error("auth-type is not bearer")
	}
}
