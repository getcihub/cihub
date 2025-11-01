package logger

import (
	"net/http"
	"strings"
	"time"

	"github.com/segmentio/ksuid"
	"github.com/sirupsen/logrus"
)

// Middleware provides a logging middleware.
func Middleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		id := r.Header.Get("X-Request-ID")
		if id == "" {
			r.Header.Set("X-Request-ID", ksuid.New().String())
		}

		ctx := r.Context()
		log := FromContext(ctx).WithField("request-id", r.Header.Get("X-Request-ID"))
		ctx = WithContext(ctx, log)

		start := time.Now()
		next.ServeHTTP(w, r.WithContext(ctx))
		end := time.Now()

		log.WithFields(logrus.Fields{
			"auth-type": authType(r),
			"latency":   end.Sub(start),
			"method":    r.Method,
			"remote":    r.RemoteAddr,
			"request":   r.RequestURI,
			"time":      end.Format(time.RFC3339),
		}).Debug()
	})
}

func authType(r *http.Request) string {
	authorization := r.Header.Get("Authorization")
	if strings.HasPrefix(authorization, "Bearer") {
		return "bearer"
	}
	return "cookie"
}
