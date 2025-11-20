package metric

import (
	"context"

	"github.com/prometheus/client_golang/prometheus"

	"github.com/getcihub/cihub/core"
)

var noContext = context.Background()

// InstallationCount provides metrics for registered installations.
func InstallationCount(installations core.InstallationStore) {
	prometheus.MustRegister(
		prometheus.NewGaugeFunc(prometheus.GaugeOpts{
			Name: "cihub_installation_count",
			Help: "Total number of active installations",
		}, func() float64 {
			i, _ := installations.Count(noContext)
			return float64(i)
		}),
	)
}
