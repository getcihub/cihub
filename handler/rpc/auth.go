package rpc

import (
	"net/http"

	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
)

// HandleAuthentication returns an http.HandlerFunc middleware that
// authenticates the http.Request and errors if the machine cannot be authenticated.
func HandleAuthentication(machines core.MachineStore) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			ctx := r.Context()
			log := logger.FromContext(ctx)

			token := r.Header.Get("X-CIHub-Token")
			if token == "" {
				w.WriteHeader(403)
				return
			}

			machine, err := machines.FindToken(ctx, token)
			if err != nil {
				w.WriteHeader(401)
				log.WithError(err).
					Debugln("api: cannot find machine token")
				return
			}

			log = log.WithFields(
				logrus.Fields{
					"machine.name":  machine.Name,
					"machine.owner": machine.Owner,
				},
			)

			ctx = logger.WithContext(ctx, log)
			next.ServeHTTP(w, r.WithContext(
				WithMachine(ctx, machine),
			))
		})
	}
}
