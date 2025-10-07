package rpc

import "net/http"

// HandlePing returns an http.HandlerFunc that makes an
// http.Request to ping the agent and confirm connectivity.
//
// GET /rpc/ping
func HandlePing() http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		writeOK(w)
	}
}

// write a 200 Status OK to the response header.
func writeOK(w http.ResponseWriter) {
	w.WriteHeader(http.StatusOK)
}
