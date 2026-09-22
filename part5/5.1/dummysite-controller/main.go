// dummysite-controller watches DummySite objects and creates a Deployment and a Service
// for each one.
//
// client-go only: the material's Go example is a kubebuilder/controller-runtime project
// with generated scaffolding, this is the watch loop written out by hand.
package main

import (
	"context"
	"fmt"
	"os"
	"time"

	appsv1 "k8s.io/api/apps/v1"
	corev1 "k8s.io/api/core/v1"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"k8s.io/apimachinery/pkg/runtime/schema"
	"k8s.io/apimachinery/pkg/util/intstr"
	"k8s.io/client-go/dynamic"
	"k8s.io/client-go/dynamic/dynamicinformer"
	"k8s.io/client-go/kubernetes"
	"k8s.io/client-go/rest"
	"k8s.io/client-go/tools/cache"
)

// The URL the API serves this resource at: /apis/<group>/<version>/<plural>.
var dummysiteGVR = schema.GroupVersionResource{Group: "stable.dwk", Version: "v1", Resource: "dummysites"}

func envOr(name, fallback string) string {
	if value := os.Getenv(name); value != "" {
		return value
	}
	return fallback
}

func boolPtr(value bool) *bool { return &value }

func namesFor(site *unstructured.Unstructured) (app, deployment, service string) {
	name := site.GetName()
	return "dummysite-" + name, name + "-dep", name + "-svc"
}

// The ownerReference is what lets the garbage collector do the cleanup.
func ownerReference(site *unstructured.Unstructured) []metav1.OwnerReference {
	return []metav1.OwnerReference{{
		APIVersion:         "stable.dwk/v1",
		Kind:               "DummySite",
		Name:               site.GetName(),
		UID:                site.GetUID(),
		Controller:         boolPtr(true),
		BlockOwnerDeletion: boolPtr(true),
	}}
}

func deploymentFor(site *unstructured.Unstructured, serverImage, websiteURL string) *appsv1.Deployment {
	app, deploymentName, _ := namesFor(site)
	labels := map[string]string{"app": app}

	return &appsv1.Deployment{
		ObjectMeta: metav1.ObjectMeta{
			Name:            deploymentName,
			Labels:          labels,
			OwnerReferences: ownerReference(site),
		},
		Spec: appsv1.DeploymentSpec{
			Replicas: int32Ptr(1),
			Selector: &metav1.LabelSelector{MatchLabels: labels},
			Template: corev1.PodTemplateSpec{
				ObjectMeta: metav1.ObjectMeta{Labels: labels},
				Spec: corev1.PodSpec{
					Containers: []corev1.Container{{
						Name:  "dummysite-server",
						Image: serverImage,
						Ports: []corev1.ContainerPort{{ContainerPort: 3000}},
						Env: []corev1.EnvVar{{
							Name:  "WEBSITE_URL",
							Value: websiteURL,
						}},
						ReadinessProbe: &corev1.Probe{
							ProbeHandler: corev1.ProbeHandler{
								HTTPGet: &corev1.HTTPGetAction{
									Path: "/healthz",
									Port: intstr.FromInt(3000),
								},
							},
							InitialDelaySeconds: 2,
							PeriodSeconds:       5,
						},
					}},
				},
			},
		},
	}
}

func serviceFor(site *unstructured.Unstructured) *corev1.Service {
	app, _, serviceName := namesFor(site)

	return &corev1.Service{
		ObjectMeta: metav1.ObjectMeta{
			Name:            serviceName,
			Labels:          map[string]string{"app": app},
			OwnerReferences: ownerReference(site),
		},
		Spec: corev1.ServiceSpec{
			Selector: map[string]string{"app": app},
			Ports: []corev1.ServicePort{{
				Port:       80,
				TargetPort: intstr.FromInt(3000),
			}},
		},
	}
}

func int32Ptr(value int32) *int32 { return &value }

func reconcile(ctx context.Context, clientset kubernetes.Interface, site *unstructured.Unstructured, serverImage string) {
	// The object is unstructured here, so the field is walked by name.
	websiteURL, found, err := unstructured.NestedString(site.Object, "spec", "website_url")
	if err != nil || !found {
		fmt.Printf("[add] %s/%s has no spec.website_url, skipping\n", site.GetNamespace(), site.GetName())
		return
	}
	_, deploymentName, serviceName := namesFor(site)
	fmt.Printf("[add] %s/%s -> %s\n", site.GetNamespace(), site.GetName(), websiteURL)

	if _, err := clientset.AppsV1().Deployments(site.GetNamespace()).Create(ctx, deploymentFor(site, serverImage, websiteURL), metav1.CreateOptions{}); err != nil {
		// AlreadyExists is expected after a restart: the informer re-lists what exists.
		if apierrors.IsAlreadyExists(err) {
			fmt.Printf("deployment %s already exists, leaving it alone\n", deploymentName)
		} else {
			fmt.Printf("creating deployment %s failed: %v\n", deploymentName, err)
		}
	} else {
		fmt.Printf("created deployment %s\n", deploymentName)
	}

	if _, err := clientset.CoreV1().Services(site.GetNamespace()).Create(ctx, serviceFor(site), metav1.CreateOptions{}); err != nil {
		if apierrors.IsAlreadyExists(err) {
			fmt.Printf("service %s already exists, leaving it alone\n", serviceName)
		} else {
			fmt.Printf("creating service %s failed: %v\n", serviceName, err)
		}
	} else {
		fmt.Printf("created service %s\n", serviceName)
	}
}

func main() {
	// In the cluster this reads the mounted service account.
	config, err := rest.InClusterConfig()
	if err != nil {
		fmt.Printf("cannot talk to the API server: %v\n", err)
		os.Exit(1)
	}
	clientset, err := kubernetes.NewForConfig(config)
	if err != nil {
		fmt.Printf("cannot build the typed client: %v\n", err)
		os.Exit(1)
	}
	dynamicClient, err := dynamic.NewForConfig(config)
	if err != nil {
		fmt.Printf("cannot build the dynamic client: %v\n", err)
		os.Exit(1)
	}

	serverImage := envOr("SERVER_IMAGE", "dummysite-server:5.1")
	ctx := context.Background()
	stopCh := make(chan struct{})
	defer close(stopCh)

	// A shared informer lists the objects and keeps the watch open, reconnecting on its own.
	factory := dynamicinformer.NewFilteredDynamicSharedInformerFactory(dynamicClient, 30*time.Second, metav1.NamespaceAll, nil)
	informer := factory.ForResource(dummysiteGVR).Informer()

	informer.AddEventHandler(cache.ResourceEventHandlerFuncs{
		AddFunc: func(obj interface{}) {
			if site, ok := obj.(*unstructured.Unstructured); ok {
				reconcile(ctx, clientset, site, serverImage)
			}
		},
		DeleteFunc: func(obj interface{}) {
			// A delete can arrive as a tombstone when the watch missed the final state.
			if tombstone, ok := obj.(cache.DeletedFinalStateUnknown); ok {
				obj = tombstone.Obj
			}
			if site, ok := obj.(*unstructured.Unstructured); ok {
				fmt.Printf("[delete] %s/%s — its Deployment and Service follow it out (ownerReferences)\n", site.GetNamespace(), site.GetName())
			}
		},
	})

	fmt.Println("watching dummysites.stable.dwk for DummySite objects")
	factory.Start(stopCh)
	if !cache.WaitForCacheSync(stopCh, informer.HasSynced) {
		fmt.Println("the informer never synced")
		os.Exit(1)
	}
	<-stopCh
}