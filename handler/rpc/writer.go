package rpc

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"

	"github.com/getcihub/cihub/store/shared/db"
)

// write a 200 Status OK to the response body.
func writeJSON(w http.ResponseWriter, v interface{}) {
	json.NewEncoder(w).Encode(v)
}

// write a 200 Status OK to the response body.
func writeOK(w http.ResponseWriter) {
	w.WriteHeader(http.StatusOK)
}

// write an error message to the response body.
func writeError(w http.ResponseWriter, err error) {
	if errors.Is(err, context.DeadlineExceeded) {
		w.WriteHeader(524) // should retry
	} else if errors.Is(err, context.Canceled) {
		w.WriteHeader(524) // should retry
	} else if errors.Is(err, db.ErrOptimisticLock) {
		w.WriteHeader(409) // should abort
	} else {
		w.WriteHeader(400) // should fail
	}

	io.WriteString(w, err.Error())
}
