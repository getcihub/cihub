package render

import (
	"encoding/json"
	"net/http"
	"os"
	"strconv"
)

// indent the json-encoded API responses
var indent bool

func init() {
	indent, _ = strconv.ParseBool(
		os.Getenv("HTTP_JSON_INDENT"),
	)
}

// Reason codes for API responses
const (
	// Success reasons
	ReasonResolved = "resolved"
	ReasonListed   = "listed"
	ReasonCreated  = "created"
	ReasonUpdated  = "updated"
	ReasonDeleted  = "deleted"
	ReasonIgnored  = "ignored"

	// Error reasons
	ReasonUnauthorized   = "unauthorized"
	ReasonForbidden      = "forbidden"
	ReasonNotFound       = "not_found"
	ReasonUserNotFound   = "user_not_found"
	ReasonBadRequest     = "bad_request"
	ReasonInternalError  = "internal_error"
	ReasonNotImplemented = "not_implemented"
	ReasonInvalidToken   = "invalid_token"
)

// Response represents a Crisp-style API response
type Response struct {
	Error  bool        `json:"error"`
	Reason string      `json:"reason"`
	Data   interface{} `json:"data"`
}

// PaginatedResponse represents a paginated API response
type PaginatedResponse struct {
	Error   bool        `json:"error"`
	Reason  string      `json:"reason"`
	HasMore bool        `json:"has_more"`
	Data    interface{} `json:"data"`
}

// Paginated writes a success response with pagination info
func Paginated(w http.ResponseWriter, data interface{}, hasMore bool) {
	write(w, &PaginatedResponse{
		Error:   false,
		Reason:  ReasonListed,
		HasMore: hasMore,
		Data:    data,
	}, 200)
}

// Success writes a success response with the given reason code
func Success(w http.ResponseWriter, reason string, data interface{}, status int) {
	write(w, &Response{
		Error:  false,
		Reason: reason,
		Data:   data,
	}, status)
}

// Error writes an error response with the given reason code
func Error(w http.ResponseWriter, reason string, status int) {
	write(w, &Response{
		Error:  true,
		Reason: reason,
		Data:   nil,
	}, status)
}

// OK writes a success response with 200 status and custom reason
func OK(w http.ResponseWriter, reason string, data interface{}) {
	Success(w, reason, data, 200)
}

// Created writes a success response with 201 status
func Created(w http.ResponseWriter, data interface{}) {
	Success(w, ReasonCreated, data, 201)
}

// InternalError writes error with 500 status
func InternalError(w http.ResponseWriter) {
	Error(w, ReasonInternalError, 500)
}

// InternalErrorf writes formatted error with 500 status
func InternalErrorf(w http.ResponseWriter, format string, a ...interface{}) {
	// Log the actual error but return generic reason
	Error(w, ReasonInternalError, 500)
}

// NotImplemented writes error with 501 status
func NotImplemented(w http.ResponseWriter) {
	Error(w, ReasonNotImplemented, 501)
}

// NotFound writes error with 404 status
func NotFound(w http.ResponseWriter) {
	Error(w, ReasonNotFound, 404)
}

// NotFoundf writes formatted error with 404 status
func NotFoundWithReason(w http.ResponseWriter, reason string) {
	Error(w, reason, 404)
}

// Unauthorized writes error with 401 status
func Unauthorized(w http.ResponseWriter) {
	Error(w, ReasonUnauthorized, 401)
}

// UnauthorizedWithReason writes error with 401 status and custom reason
func UnauthorizedWithReason(w http.ResponseWriter, reason string) {
	Error(w, reason, 401)
}

// Forbidden writes error with 403 status
func Forbidden(w http.ResponseWriter) {
	Error(w, ReasonForbidden, 403)
}

// BadRequest writes error with 400 status
func BadRequest(w http.ResponseWriter) {
	Error(w, ReasonBadRequest, 400)
}

// BadRequestWithReason writes error with 400 status and custom reason
func BadRequestWithReason(w http.ResponseWriter, reason string) {
	Error(w, reason, 400)
}

func write(w http.ResponseWriter, v interface{}, status int) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	enc := json.NewEncoder(w)
	if indent {
		enc.SetIndent("", "  ")
	}
	_ = enc.Encode(v)
}
