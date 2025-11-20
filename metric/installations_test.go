package metric

import (
	"testing"

	"go.uber.org/mock/gomock"

	"github.com/getcihub/cihub/mock"

	"github.com/prometheus/client_golang/prometheus"
)

func TestUserCount(t *testing.T) {
	controller := gomock.NewController(t)

	// restore the default prometheus registerer
	// when the unit test is complete.
	snapshot := prometheus.DefaultRegisterer
	defer func() {
		prometheus.DefaultRegisterer = snapshot
		controller.Finish()
	}()

	// creates a blank registry
	registry := prometheus.NewRegistry()
	prometheus.DefaultRegisterer = registry

	// x2 installation count
	count := int64(5)

	store := mock.NewMockInstallationStore(controller)
	store.EXPECT().Count(gomock.Any()).Return(count, nil)
	InstallationCount(store)

	metrics, err := registry.Gather()
	if err != nil {
		t.Error(err)
		return
	}
	if want, got := len(metrics), 1; want != got {
		t.Errorf("Expect registered metric")
		return
	}
	metric := metrics[0]
	if want, got := metric.GetName(), "cihub_installation_count"; want != got {
		t.Errorf("Expect metric name %s, got %s", want, got)
	}
	if want, got := metric.Metric[0].Gauge.GetValue(), float64(count); want != got {
		t.Errorf("Expect metric value %f, got %f", want, got)
	}
}
