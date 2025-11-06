package rpc

import (
	"net/http"
	"time"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
)

// HandleJoin returns an http.HandlerFunc that makes an
// http.Request to join the cluster.
//
// POST /rpc/v1/join
func HandleJoin(machines core.MachineStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		ctx := r.Context()
		machine, _ := MachineFrom(ctx)
		log := logger.FromContext(ctx)

		machine.LastSeen = time.Now().Unix()
		machine.Status = core.MachineStatusOnline
		machine.Updated = time.Now().Unix()

		err := machines.Update(ctx, machine)
		if err != nil {
			writeError(w, err)
			log.WithError(err).
				Warnln("rpc: machine cannot join cluster")
			return
		}

		log.Debugln("rpc: machine joined cluster")

		writeOK(w)
	}
}
