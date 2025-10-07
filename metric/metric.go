package metric

import (
	"errors"
	"net/http"

	"github.com/prometheus/client_golang/prometheus/promhttp"
)

// errInvalidToken is returned when the prometheus token is invalid.
var errInvalidToken = errors.New("invalid or missing prometheus token")

// errAccessDenied is returned when the authorized user does not
// have access to the metrics endpoint.
var errAccessDenied = errors.New("access denied")

func HandleMetrics(token string) http.HandlerFunc {
	handler := promhttp.Handler()
	return func(w http.ResponseWriter, r *http.Request) {
		// if a bearer token is not configured we should
		// just server the http request.
		if token == "" {
			handler.ServeHTTP(w, r)
			return
		}

		header := r.Header.Get("Authorization")
		if header == "" {
			http.Error(w, errAccessDenied.Error(), http.StatusUnauthorized)
			return
		}

		if header != "Bearer "+token {
			http.Error(w, errInvalidToken.Error(), http.StatusForbidden)
			return
		}

		handler.ServeHTTP(w, r)
	}
}
