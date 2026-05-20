# SW/HW Co-Design Report

# SW/HW Co-Design and Co-Creation Principles for AI in the Edge-Cloud Continuum

**Technical Report**  
**Anonymized neuromorphic FPGA project**
**Scope: Networking, orchestration and deployment of AI models**
**Version 2.0**
**October 30, 2025**

---

## Table of Contents

*[Table of Contents would be automatically generated based on the sections below]*

---

## Executive Summary

This report presents a comprehensive framework for hardware-software co-design and co-creation principles specifically tailored for artificial intelligence systems operating in the edge-cloud continuum. The work has been conducted under the auspices of the anonymized neuromorphic FPGA project, which aims to advance brain-inspired neuromorphic computing systems through innovative orchestration, deployment, and operational methodologies.

The edge-cloud continuum represents a paradigm shift in distributed computing architecture, enabling seamless integration between resource-constrained edge devices and powerful cloud infrastructure. This architectural model is particularly relevant for neuromorphic AI workloads, which demand specialized hardware acceleration through Field-Programmable Gate Arrays (FPGAs) while maintaining operational flexibility across heterogeneous deployment environments. The co-design approach presented herein addresses the fundamental challenge of partitioning computational workloads, managing hardware resources, and orchestrating distributed AI systems in a manner that optimizes performance, energy efficiency, and operational reliability.

The orchestration framework leverages Kubernetes as the foundational platform, extended with specialized components including custom device plugins for FPGA resource management, K3s distributions for resource-constrained edge deployments, and optional KubeEdge extensions for enhanced edge autonomy. The architecture implements a five-layer design pattern encompassing application services, orchestration mechanisms, container/WebAssembly runtimes, hardware abstraction, and physical infrastructure. This layered approach facilitates clean separation of concerns while maintaining well-defined interfaces for cross-layer communication and resource management.

Central to the co-design methodology is the iterative refinement process that bridges software algorithm development with hardware implementation constraints. The framework incorporates human-centric co-creation principles, ensuring that system design decisions reflect the requirements, concerns, and operational realities of end users, developers, and domain experts. This participatory approach extends beyond traditional requirements engineering to encompass ongoing stakeholder engagement throughout the design, implementation, and validation phases.

The report establishes technical specifications for implementing production-ready orchestration systems, including detailed deployment workflows, security frameworks implementing defense-in-depth principles, and operational best practices for managing FPGA-accelerated workloads. Particular emphasis is placed on the Pareto-optimal design philosophy, which prioritizes core functionality that delivers substantial operational value while deferring complex features until foundational capabilities are proven stable in production environments.

---

## 1. Introduction and Background

### 1.1 Motivation and Context

The proliferation of artificial intelligence applications across diverse domains has necessitated a fundamental rethinking of computational architecture and deployment strategies. Traditional cloud-centric AI paradigms, while offering substantial computational resources, impose latency penalties and bandwidth constraints that prove prohibitive for time-critical applications such as autonomous systems, industrial automation, and real-time decision support. Conversely, pure edge computing approaches struggle with resource limitations and management complexity when scaling beyond isolated deployments.

The edge-cloud continuum emerges as a synthesis of these complementary paradigms, enabling adaptive workload placement that leverages the strengths of both architectural extremes. This continuum model aligns naturally with neuromorphic computing principles, which emphasize distributed processing, event-driven computation, and energy-efficient operation. Neuromorphic systems, inspired by biological neural architectures, represent a departure from traditional von Neumann computing models and require specialized hardware implementations to realize their computational advantages.

The anonymized neuromorphic FPGA project specifically targets the integration of brain-inspired computing paradigms including Spiking Neural Networks (SNNs) and Bayesian Confidence Propagation Neural Networks (BCPNN) with FPGA-based hardware acceleration platforms. FPGAs offer several advantages for neuromorphic implementations: reconfigurability enabling algorithmic flexibility, parallel processing capabilities matching the distributed nature of neural computation, and power efficiency approaching that of dedicated Application-Specific Integrated Circuits (ASICs) while maintaining post-deployment adaptability.

### 1.2 Challenges in Edge-Cloud AI Orchestration

The deployment of neuromorphic AI systems across the edge-cloud continuum presents several interconnected technical challenges that necessitate a co-design approach:

- **Hardware Heterogeneity**: Edge nodes may incorporate diverse FPGA platforms (Xilinx Zynq UltraScale+, Intel Stratix, etc.) with varying resource profiles, while cloud infrastructure provides general-purpose compute alongside potential GPU or ASIC acceleration. Abstracting this heterogeneity while preserving hardware-specific optimization capabilities requires careful architectural design.

- **Resource Management Complexity**: FPGAs present unique resource management challenges distinct from CPU/GPU allocation. Bitstream loading times, partial reconfiguration regions, FPGA-specific memory hierarchies, and device-level constraints necessitate specialized orchestration mechanisms beyond standard container runtime capabilities.

- **Network Connectivity Variability**: Edge deployments frequently experience intermittent connectivity to cloud infrastructure due to network constraints, mobility, or intentional air-gapping for security. Orchestration systems must accommodate these connectivity patterns while maintaining operational consistency and enabling local decision-making during network partitions.

- **Security and Isolation**: Hardware-accelerated workloads require elevated privileges for device access, creating potential security vulnerabilities. Balancing the need for hardware access with container isolation, principle of least privilege, and defense-in-depth security postures represents a critical design constraint.

- **Operational Complexity**: Managing distributed FPGA-accelerated workloads introduces operational overhead including bitstream versioning, driver stack synchronization, hardware health monitoring, and failure recovery mechanisms. Minimizing this operational burden while maintaining system reliability requires thoughtful automation and abstraction.

### 1.3 Objectives and Contributions

This report presents a comprehensive technical framework addressing the aforementioned challenges through principled hardware-software co-design and co-creation methodologies. The primary contributions include:

- **Architectural Framework**: A five-layer architecture design that cleanly separates concerns across application logic, orchestration mechanisms, runtime environments, hardware abstraction, and physical infrastructure. This layered approach enables independent evolution of components while maintaining well-defined interfaces.

- **FPGA Integration Methodology**: Detailed specifications for integrating FPGA resources into Kubernetes orchestration through custom device plugins, extending the standard device plugin framework to accommodate FPGA-specific requirements including bitstream management, resource discovery, and health monitoring.

- **Co-Design Process**: An iterative methodology for concurrent hardware and software development that addresses algorithmic requirements, hardware implementation constraints, performance optimization, and validation through continuous feedback loops.

- **Security Framework**: Multi-layer security architecture implementing defense-in-depth principles across infrastructure, network, container, and application layers, with specific attention to the elevated privilege requirements of hardware access.

- **Implementation Guidance**: Production-ready specifications including deployment workflows, CI/CD integration patterns, operational best practices, and concrete implementation examples targeting Xilinx ZCU104 FPGA platforms.

---

## 2. Theoretical Framework and Co-Design Principles

### 2.1 Hardware-Software Co-Design Methodology

Hardware-software co-design represents a departure from traditional sequential development processes where hardware platforms are specified first and software subsequently adapted to hardware constraints. Instead, co-design embraces concurrent, iterative refinement of both hardware and software components, enabling mutual influence and optimization opportunities that would be precluded by sequential approaches.

The theoretical foundation of co-design rests on recognizing that optimal system performance emerges from holistic consideration of the entire computational stack rather than isolated optimization of individual layers. For neuromorphic AI systems, this principle manifests in several key dimensions:

#### 2.1.1 Algorithmic-Hardware Co-Optimization

Neural network architectures and learning algorithms possess inherent computational characteristics including memory access patterns, arithmetic operation distributions, and data dependency structures. Hardware implementations can be optimized to exploit these characteristics through specialized arithmetic units, custom memory hierarchies, and datapath configurations. Conversely, algorithmic design choices regarding network topology, activation functions, and precision requirements directly impact hardware resource utilization and performance metrics.

The co-optimization process proceeds through iterative cycles where algorithm developers specify computational requirements including throughput targets, latency bounds, and accuracy constraints. Hardware architects respond with resource estimates, timing analysis, and power projections based on Register Transfer Level (RTL) designs. This bidirectional feedback enables algorithm modifications that improve hardware efficiency (e.g., reduced precision, structured sparsity) while hardware refinements accommodate algorithm-critical operations (e.g., specialized activation function units).

#### 2.1.2 Parameterizable Hardware Architectures

FPGA-based neuromorphic implementations benefit from parameterizable design patterns that enable post-synthesis configuration of key architectural parameters. Relevant parameters include:

- **Numerical Representation**: Bit widths for neuron states, synaptic weights, and intermediate computations directly impact resource utilization and accuracy. Parameterization enables exploration of the precision-performance tradeoff space.

- **Parallelism Degree**: The number of parallel processing elements determines throughput and resource consumption. Architectural parameterization allows adaptation to available FPGA resources and workload characteristics.

- **Clock Frequency**: Dynamic frequency scaling enables energy-performance tradeoffs based on workload urgency and thermal constraints.

- **Memory Hierarchy Configuration**: On-chip memory allocation between various caches and buffers impacts bandwidth utilization and access latency.

### 2.2 Co-Creation Principles for Human-Centric AI

Beyond technical co-design considerations, the anonymized neuromorphic FPGA project embraces co-creation methodologies that integrate human stakeholders throughout the system development lifecycle. Co-creation extends traditional user-centered design by positioning users, domain experts, and other stakeholders as active participants in design decisions rather than passive requirement sources or validation subjects.

#### 2.2.1 Stakeholder Engagement Framework

The stakeholder engagement framework incorporates multiple interaction modalities including co-design workshops, living labs, iterative prototyping sessions, and continuous feedback mechanisms. Workshop participants represent diverse perspectives spanning end users from application domains, developers responsible for system implementation, and social scientists bringing expertise in ethical, legal, and social implications of AI systems.

Workshop activities focus on eliciting values, concerns, and requirements regarding trustworthiness, explainability, and socio-technical integration of brain-inspired AI systems. Structured exercises facilitate articulation of often implicit expectations about system behavior, acceptable failure modes, explanation quality, and human-AI collaboration patterns. The insights gathered inform both technical design decisions (e.g., explainability interface requirements, performance-accuracy tradeoffs) and operational policies (e.g., human oversight protocols, fallback procedures).

#### 2.2.2 Trustworthy AI Requirements

Trustworthy AI emerges as a central co-creation principle, encompassing several interrelated dimensions that must be addressed through both technical mechanisms and socio-technical processes:

- **Transparency**: The ability for stakeholders to understand how AI systems operate, including data sources, algorithmic logic, decision-making processes, and performance characteristics. Transparency mechanisms include model documentation, inference logging, decision provenance tracking, and accessible explanations.

- **Explainability**: Technical capabilities enabling generation of human-understandable explanations for system decisions. For neuromorphic systems, explainability presents unique challenges due to the distributed, event-driven nature of computation. The co-design process must anticipate explainability requirements and incorporate appropriate instrumentation.

- **Fairness and Bias Mitigation**: Systematic approaches to identifying and mitigating sources of bias in training data, algorithmic design, and deployment contexts. Co-creation workshops engage stakeholders in defining fairness criteria appropriate to specific application domains.

- **Accountability**: Mechanisms for attributing responsibility for system behavior and establishing audit trails. Technical implementations include comprehensive logging, version control of models and configurations, and organizational processes defining roles and responsibilities.

### 2.3 Co-Design and Co-Creation Framework Diagram

Figure 1 illustrates the comprehensive co-design and co-creation framework, showing the interconnection between technical co-design principles (hardware-software integration, performance optimization, modular architecture) and human-centric co-creation principles (trustworthy AI, iterative refinement, human-centric design).

![Figure 1: SW/HW Co-Design and Co-Creation Principles Framework](figure-1-placeholder.png)

*Figure 1: SW/HW Co-Design and Co-Creation Principles Framework*

---

## 3. System Architecture for Edge-Cloud AI Orchestration

### 3.1 Five-Layer Architectural Model

The orchestration architecture implements a five-layer design pattern that provides clean separation of concerns while maintaining well-defined interfaces for cross-layer communication. Each layer encapsulates specific functionality and abstracts implementation details from adjacent layers, enabling independent evolution and technology substitution within layers without disrupting the overall system.

![Figure 2: Five-Layer Architecture for Edge-Cloud AI Orchestration](figure-2-placeholder.png)

*Figure 2: Five-Layer Architecture for Edge-Cloud AI Orchestration*

#### 3.1.1 Application Layer

The application layer contains neuromorphic AI services, management APIs, and telemetry systems that comprise the user-facing functionality. Applications at this layer interact with lower layers exclusively through defined APIs and service interfaces, remaining agnostic to underlying orchestration mechanisms and hardware implementations. Key components include:

- **Inference Services**: REST/gRPC APIs exposing neural network inference capabilities, accepting input data and returning predictions, classifications, or other domain-specific outputs.

- **Management Interfaces**: Control plane APIs enabling model deployment, configuration updates, monitoring query, and lifecycle management operations.

- **Telemetry Collection**: Instrumentation for performance metrics, inference latency, accuracy measurements, and resource utilization statistics.

#### 3.1.2 Orchestration Layer

The orchestration layer provides the core container and workload management capabilities, implementing scheduling logic, resource allocation, service discovery, and lifecycle management. This layer bridges between high-level application requirements and low-level resource provisioning.

The orchestration layer is built upon Kubernetes for cloud control plane and K3s for edge nodes. Kubernetes was selected due to its maturity, extensive ecosystem, and proven scalability in production environments. K3s provides a lightweight Kubernetes distribution optimized for resource-constrained edge deployments, reducing memory footprint from approximately 1GB to 512MB while maintaining API compatibility.

Key orchestration layer components include:

- **Kubernetes Control Plane**: Centralized management comprising the API server (cluster management interface), etcd (distributed state store), scheduler (workload placement), and controller manager (reconciliation loops for desired state enforcement).

- **Device Plugin Framework**: Kubernetes extension mechanism enabling custom hardware resource types. The FPGA device plugin implements discovery, advertisement, allocation, and health monitoring for FPGA resources.

- **Node Feature Discovery (NFD)**: Automatic detection and labeling of node capabilities including hardware features, driver versions, and available accelerators, enabling scheduler affinity rules and workload targeting.

- **KubeEdge Extensions (Optional)**: Edge-specific enhancements providing cloud-edge messaging, edge autonomy during disconnection, device management abstractions, and edge-side caching of container images and configuration.

#### 3.1.3 Container and WebAssembly Runtime Layer

The runtime layer provides execution environments for application workloads, supporting both traditional OCI (Open Container Initiative) containers and lightweight WebAssembly modules. This dual-runtime approach enables optimization for different deployment scenarios:

- **OCI Containers**: Full-featured container runtime via containerd or CRI-O, suitable for complex applications requiring complete filesystem access, extensive dependencies, and rich system integration. Containers provide strong isolation through Linux namespaces and cgroups while maintaining compatibility with existing containerized applications.

- **WebAssembly Runtime**: WasmEdge or Wasmtime runtimes for ultra-lightweight workloads requiring minimal startup latency and memory footprint. WebAssembly provides sandbox isolation with near-native execution performance, making it suitable for edge preprocessing, data filtering, and simple inference tasks.

#### 3.1.4 Hardware Abstraction Layer

The hardware abstraction layer masks the heterogeneity of underlying physical resources, presenting uniform interfaces to higher layers while managing device-specific drivers, firmware, and configuration. Components include:

- **FPGA Drivers (zocl)**: Kernel-level drivers managing FPGA device access, DMA operations, and interrupt handling. The zocl driver specifically targets Xilinx Zynq UltraScale+ devices.

- **Xilinx Runtime (XRT)**: User-space runtime library providing APIs for bitstream programming, buffer management, kernel execution, and device monitoring. XRT abstracts low-level hardware interfaces while exposing necessary control for optimization.

- **Memory Management**: Contiguous Memory Allocator (CMA) configuration for large buffer allocation, System MMU (SMMU) or Input-Output Memory Management Unit (IOMMU) setup for safe DMA operations with address translation and protection.

#### 3.1.5 Hardware Infrastructure Layer

The infrastructure layer comprises physical computing resources, network fabric, and supporting systems:

- **FPGA Nodes**: Xilinx ZCU104 evaluation boards featuring Zynq UltraScale+ MPSoC devices with quad-core ARM Cortex-A53 processor system and programmable logic fabric. Resource specifications include 230,400 LUTs, 460,800 flip-flops, 663 DSP slices, and 312 Block RAMs.

- **General-Purpose Nodes**: CPU-only compute nodes for control plane services, non-accelerated workloads, and auxiliary services.

- **Network Infrastructure**: Gigabit Ethernet connectivity with optional Time-Sensitive Networking (TSN) extensions for deterministic latency requirements. Cloud interconnection via secure VPN or dedicated circuits.

### 3.2 Edge-Cloud Continuum Architecture

Figure 3 depicts the distributed architecture spanning cloud control plane and multiple edge sites. The cloud hosts the Kubernetes control plane components while edge sites run K3s agents with FPGA-equipped nodes. Secure network connections enable bidirectional control plane communication, workload deployment, and telemetry aggregation.

![Figure 3: Edge-Cloud Continuum Architecture for Neuromorphic AI](figure-3-placeholder.png)

*Figure 3: Edge-Cloud Continuum Architecture for Neuromorphic AI*

The architecture implements several key design patterns that enable seamless operation across the edge-cloud continuum:

#### 3.2.1 Unified Control Plane

All nodes, regardless of physical location (cloud or edge), register with a single Kubernetes control plane. This unified management approach enables consistent workload definitions, policy enforcement, and operational procedures across the entire infrastructure. Operators interact with a single API server for cluster-wide operations, eliminating the need for site-specific tools or procedures.

#### 3.2.2 Topology-Aware Scheduling

Node labels encode topological information including geographical region, network zone, and hardware capabilities. The scheduler uses this metadata to make intelligent placement decisions that account for latency requirements, bandwidth constraints, and hardware availability. Affinity and anti-affinity rules enable sophisticated placement strategies such as co-locating communicating services or distributing replicas across failure domains.

#### 3.2.3 Secure Communication Patterns

All control plane communications utilize mutual TLS (mTLS) with certificate-based authentication. Edge nodes maintain persistent gRPC connections to the API server, carrying bidirectional state synchronization and deployment instructions. Data plane communications employ Kubernetes Services abstraction with appropriate exposure models (ClusterIP for internal, NodePort for edge-external, LoadBalancer for cloud-external).

---

## 4. Hardware-Software Co-Design Implementation

### 4.1 Iterative Co-Design Process

The co-design methodology implements an iterative refinement cycle that bridges algorithmic development, hardware implementation, integration, and validation phases. Figure 4 illustrates this process flow with explicit feedback loops enabling continuous optimization based on empirical measurements and evolving requirements.

![Figure 4: Hardware-Software Co-Design Methodology](figure-4-placeholder.png)

*Figure 4: Hardware-Software Co-Design Methodology*

#### 4.1.1 Requirements Analysis Phase

The process initiates with comprehensive requirements gathering incorporating functional specifications, performance targets, power budgets, and operational constraints. Requirements emerge from multiple sources:

- **Application Domain Analysis**: Understanding use case requirements including inference latency bounds, throughput targets, accuracy expectations, and environmental constraints (power availability, cooling, physical space).

- **Algorithmic Profiling**: Characterizing computational patterns through software profiling: operation distributions (multiply-accumulate, activations, memory transfers), memory access patterns (sequential, random, streaming), and computational intensity (ratio of operations to memory accesses).

- **Hardware Platform Constraints**: Available FPGA resources (logic capacity, memory, DSP blocks), physical interfaces (PCIe lanes, network ports), and development tool capabilities.

#### 4.1.2 Architecture Specification Phase

Based on requirements, architects develop candidate system architectures exploring different partitioning strategies between software and hardware components. Key decisions include:

- **Functional Partitioning**: Which operations execute in programmable logic versus processor system? Typically, compute-intensive kernels (convolution, matrix multiplication, activation) map to FPGA while control flow, host interface, and irregular operations remain in software.

- **Communication Interface Design**: Definition of host-FPGA communication mechanisms including DMA buffer structures, control register mappings, interrupt signaling, and synchronization protocols.

- **Memory Hierarchy**: Allocation of on-chip BRAM for critical data structures, off-chip DDR for model parameters and large datasets, streaming interfaces for high-bandwidth data flows.

### 4.2 FPGA Implementation Considerations

#### 4.2.1 Register Transfer Level Design

Neuromorphic accelerators are implemented at Register Transfer Level (RTL) using hardware description languages (Verilog, VHDL, or high-level synthesis from C++). The RTL design process encompasses:

- **Datapath Architecture**: Design of arithmetic units, register files, and data routing networks implementing neural network operations. Spiking neural networks require event-driven processing with timestamp management, membrane potential accumulators, and threshold comparison circuits.

- **Control Logic**: Finite state machines coordinating computation phases, memory transfers, and host communication. Control logic must handle edge cases including buffer full/empty conditions, error states, and synchronization events.

- **Timing Optimization**: Pipeline insertion, register balancing, and critical path optimization to achieve target clock frequencies. Timing closure for neuromorphic designs often requires careful attention to routing resources and placement constraints.

#### 4.2.2 Resource Utilization Analysis

FPGA resource utilization directly impacts achievable parallelism and ultimately system performance. Resource types include:

- **Logic Resources**: Look-Up Tables (LUTs) implement combinational logic, flip-flops provide state storage. The Xilinx ZCU104 provides 230,400 LUTs and 460,800 flip-flops, constraining the maximum complexity of implementable designs.

- **DSP Slices**: Dedicated multiplier-accumulator units (663 DSP48E2 slices on ZCU104) provide high-performance arithmetic operations. Efficient DSP utilization is critical for computational throughput.

- **Block RAM**: On-chip memory blocks (312×36Kb on ZCU104, totaling 10.8 Mb) store intermediate results, weights, and activation values. Memory partitioning and banking strategies significantly impact bandwidth and latency.

---

## 5. Orchestration and Deployment Framework

### 5.1 FPGA Device Plugin Architecture

The FPGA device plugin extends Kubernetes' device plugin framework to support FPGA resource management. The plugin runs as a privileged DaemonSet on each FPGA-equipped node, implementing the device plugin gRPC API to enable FPGA discovery, advertisement, allocation, and health monitoring.

![Figure 5: FPGA Device Plugin Integration Architecture](figure-5-placeholder.png)

*Figure 5: FPGA Device Plugin Integration Architecture*

#### 5.1.1 Device Discovery and Registration

Upon startup, the device plugin scans the system for FPGA devices by examining Linux sysfs entries (/sys/class/fpga_manager/) and enumerating user-space I/O devices (/dev/uio*). The plugin identifies Xilinx FPGA manager instances and associated UIO devices through device tree matching and driver name inspection.

After discovering available FPGAs, the plugin registers with the kubelet via the device plugin socket (/var/lib/kubelet/device-plugins/kubelet.sock). Registration includes the resource name (e.g., xilinx.com/fpga-zcu104) and Unix socket endpoint for subsequent gRPC communication. The kubelet acknowledges registration and begins tracking the advertised extended resource.

#### 5.1.2 Resource Advertisement via ListAndWatch

The device plugin implements the ListAndWatch gRPC streaming endpoint, through which it continuously reports available devices and their health status to the kubelet. The plugin maintains an internal device list, initially populated during discovery and subsequently updated based on health checks.

Health monitoring proceeds through periodic polling of FPGA manager state attributes and UIO device accessibility. The plugin reads /sys/class/fpga_manager/fpga0/state, expecting 'operating' status for healthy devices. Device file accessibility (/dev/uio0) is verified through stat() system calls. Upon detecting health state changes, the plugin updates the ListAndWatch stream, prompting the kubelet to adjust node capacity and allocatable resources accordingly.

#### 5.1.3 Device Allocation Process

When the Kubernetes scheduler assigns a pod requesting FPGA resources to a node, the kubelet invokes the device plugin's Allocate method. The method receives the list of device IDs to allocate and returns allocation responses specifying:

- **Device Files**: Paths to device nodes (/dev/uio0, /dev/dri/renderD128) that should be mounted into the container namespace with appropriate permissions.

- **Environment Variables**: Variables conveying device information to container workloads, such as FPGA_DEVICE_PATH, FPGA_BITSTREAM_NAME, enabling dynamic configuration.

- **Mount Points**: Host path mounts for firmware directories or shared libraries required for FPGA access.

#### 5.1.4 Bitstream Programming via PreStartContainer

The device plugin optionally implements the PreStartContainer hook, enabling FPGA bitstream programming immediately before container launch. This approach ensures the FPGA is configured with the appropriate accelerator function before the workload begins execution.

Bitstream programming utilizes the Linux FPGA Manager framework, which provides a standardized sysfs interface for FPGA configuration. The plugin writes the bitstream filename to /sys/class/fpga_manager/fpga0/firmware, triggering kernel-level bitstream loading. The operation blocks until programming completes, typically requiring 10-100 milliseconds depending on bitstream size. The plugin verifies successful programming by reading the state attribute, ensuring it reports 'operating' status before allowing container startup.

### 5.2 End-to-End Deployment Workflow

Figure 6 illustrates the complete workflow from infrastructure provisioning through runtime execution, highlighting the interaction between various system components and the sequential progression through deployment phases.

![Figure 6: End-to-End Orchestration Workflow for FPGA-Accelerated AI Workloads](figure-6-placeholder.png)

*Figure 6: End-to-End Orchestration Workflow for FPGA-Accelerated AI Workloads*

#### 5.2.1 Phase 1: Infrastructure Setup

Infrastructure provisioning establishes the foundational compute, network, and storage resources required for orchestration:

1. **Edge Node Provisioning**: Installation of base operating system (PetaLinux 2023.1 or Ubuntu 22.04 LTS for ARM64), FPGA drivers (zocl kernel module), and Xilinx Runtime (XRT) libraries. Kernel boot parameters must allocate sufficient contiguous memory via CMA (cma=512M minimum) for DMA operations.

2. **Node Registration**: Execution of K3s agent join command with control plane endpoint and authentication token. Node transitions to Ready state after successful kubelet startup and health verification.

3. **Device Plugin Deployment**: Application of DaemonSet manifest deploying the FPGA device plugin to nodes labeled hardware.fpga.local/fpga=true. The device plugin registers with kubelet and begins advertising FPGA resources.

4. **Resource Verification**: Confirmation that extended resources (xilinx.com/fpga-zcu104) appear in node capacity and allocatable fields via kubectl describe node commands.

#### 5.2.2 Phase 2: Application Deployment

Application deployment proceeds through automated CI/CD pipelines that handle build, test, and release operations:

5. **Helm Chart Preparation**: Definition of Kubernetes resources (Deployment, Service, ConfigMap) with FPGA resource requests specified in pod specifications. Node selectors and affinity rules target FPGA-equipped nodes.

6. **CI/CD Pipeline Execution**: Git commit triggers automated pipeline: container image build incorporating application code and dependencies, image push to registry with semantic versioning, Helm deployment to target namespace.

7. **Scheduler Operation**: Kubernetes scheduler evaluates pending pods, identifies nodes with available FPGA resources matching request quantities, and performs pod binding to selected nodes.

8. **Resource Allocation**: Device plugin Allocate method invoked, returns device files and environment variables. Kubelet configures container runtime with allocation response.

#### 5.2.3 Phase 3: Runtime Execution

Runtime execution encompasses container initialization, bitstream loading, and workload operation:

9. **Container Initialization**: Container runtime creates namespace, mounts device files (/dev/uio0), injects environment variables, and starts container process with specified user context and security constraints.

10. **Bitstream Loading**: Application initializes XRT runtime, opens FPGA device, loads bitstream (.xclbin file) specifying accelerator function. XRT manages FPGA programming through kernel driver interface.

11. **Workload Execution**: Application allocates DMA buffers, transfers input data, initiates FPGA kernel execution, retrieves results. Continuous operation processes inference requests with FPGA acceleration.

---

## 6. Security and Operational Considerations

### 6.1 Multi-Layer Security Architecture

Security is embedded throughout the orchestration architecture following defense-in-depth principles. The security framework implements multiple overlapping defensive layers such that compromise of any single layer does not result in complete system breach. Figure 7 illustrates this multi-layer security architecture.

![Figure 7: Multi-Layer Security Framework for Edge-Cloud AI](figure-7-placeholder.png)

*Figure 7: Multi-Layer Security Framework for Edge-Cloud AI*

#### 6.1.1 Infrastructure Security Layer

Infrastructure security encompasses control plane hardening, certificate management, and foundational access controls:

- **TLS Control Plane**: All Kubernetes API communications employ TLS 1.3 encryption with strong cipher suites. Certificate authority (CA) certificates are protected with appropriate file permissions and rotation schedules.

- **Role-Based Access Control (RBAC)**: Fine-grained authorization policies restrict API operations based on user identity and role assignments. Service accounts follow principle of least privilege, granted only necessary permissions for intended functionality.

- **Audit Logging**: Comprehensive audit logs capture all API requests including requestor identity, requested resource, operation type, and response status. Logs are streamed to centralized secure storage for analysis and compliance.

- **Secret Management**: Sensitive configuration data (API keys, certificates, credentials) stored as Kubernetes Secrets with encryption at rest enabled. Access to secrets restricted through RBAC policies.

#### 6.1.2 Network Security Layer

Network security implements microsegmentation and encrypted communications:

- **Network Policies**: Kubernetes NetworkPolicy resources define allowed communication paths between pods. Default-deny policies block unexpected traffic flows, with explicit allow rules for legitimate communication.

- **Mutual TLS (mTLS)**: Service mesh implementations (Istio, Linkerd) provide automatic mTLS for all pod-to-pod communications, ensuring encryption and authentication without application modification.

- **Ingress Control**: External traffic enters cluster through controlled ingress points with TLS termination, authentication, rate limiting, and web application firewall capabilities.

#### 6.1.3 Container Security Layer

Container-level security hardens workload execution environments:

- **Non-Root Execution**: All application containers execute with non-root user contexts (UID 1000+), preventing privilege escalation through container escape vulnerabilities.

- **Seccomp Profiles**: Secure computing mode (seccomp) profiles restrict available system calls to minimum necessary set, reducing attack surface for kernel exploits.

- **Capability Dropping**: Linux capabilities dropped from containers (capabilities.drop: [ALL]), relying on device plugin for hardware access rather than elevated container privileges.

- **Read-Only Filesystem**: Container root filesystems mounted read-only where feasible, preventing runtime modification of executables and libraries. Writable volumes provided only for legitimate data directories.

### 6.2 Operational Best Practices

#### 6.2.1 Version Control and Configuration Management

All infrastructure and application configurations are maintained in Git repositories implementing GitOps principles. Configuration changes proceed through pull request workflows with mandatory code review, automated testing, and audit trail preservation. Critical version dependencies include:

- **XRT Version Locking**: Host and container XRT versions must align exactly to ensure bitstream compatibility. CI/CD pipelines enforce version matching through build-time verification.

- **Bitstream Versioning**: FPGA bitstreams tagged with semantic versions and stored in artifact repositories with SHA-256 checksums. ConfigMaps or Custom Resources reference specific bitstream versions.

- **Container Image Immutability**: Container images tagged with specific versions (not 'latest'), ensuring reproducible deployments and enabling rollback to previous versions.

#### 6.2.2 Monitoring and Observability

Comprehensive observability enables proactive issue detection and performance optimization:

- **Metrics Collection**: Prometheus scrapes metrics from application endpoints, kubelet, device plugin, and node exporters. Custom metrics expose FPGA-specific data including utilization, temperature, power consumption, and error counters.

- **Visualization**: Grafana dashboards present system metrics with appropriate context: FPGA node health, workload scheduling efficiency, inference latency distributions, resource utilization time series.

- **Alerting**: Prometheus alerting rules trigger notifications for anomalous conditions: pod crashes, FPGA health degradation, scheduling failures, SLA violations. Alerts integrate with incident management platforms (PagerDuty, Opsgenie).

---

## 7. Implementation Strategy and Phases

### 7.1 Phased Implementation Approach

The implementation follows a Pareto-optimal strategy prioritizing core functionality that delivers substantial operational value while deferring complex features until foundational capabilities are proven stable. This approach manages risk, enables early validation of architectural decisions, and provides tangible results for stakeholder feedback.

#### 7.1.1 Phase 1: Minimum Viable Orchestration

Phase 1 establishes a functional orchestration platform supporting neuromorphic workload deployment to FPGA-equipped edge nodes with essential security, monitoring, and automation. Scope includes:

- **Infrastructure Deployment**: Kubernetes control plane in cloud environment, K3s edge cluster with 2-3 FPGA nodes (Xilinx ZCU104), basic networking connectivity.

- **FPGA Stack**: XRT installation and validation, kernel driver loading (zocl), end-to-end bitstream programming workflow verification.

- **Device Plugin**: Custom FPGA device plugin implementation, DaemonSet deployment, resource advertisement to scheduler, node labeling (hardware.fpga.local/fpga=true).

- **Workload Deployment**: Helm charts for neuromorphic service containers, FPGA resource requests, scheduling verification, XRT API access confirmation.

- **CI/CD Pipeline**: GitLab or Jenkins pipeline for container build, Helm deployment automation, resource allocation validation.

- **Monitoring**: Prometheus metrics collection, Grafana dashboards for node health, FPGA availability, pod status, resource utilization.

- **Security Hardening**: Non-root pod execution, seccomp profiles, TLS control plane, network policies for pod isolation.

**Success criteria**: Successful deployment of neuromorphic workload to FPGA node via Helm, correct FPGA resource allocation, end-to-end CI/CD execution, operational monitoring dashboards, security policy enforcement.

#### 7.1.2 Phase 2: Advanced Features

Phase 2 features are explicitly deferred until Phase 1 demonstrates stable operation in production. Implementation timing depends on operational experience and validated requirements:

- **WebAssembly Runtime**: WasmEdge/Wasmtime deployment for ultra-lightweight edge workloads, RuntimeClass configuration, WASI interfaces for FPGA access.

- **Service Mesh**: Istio or Linkerd for automatic mTLS, advanced traffic routing, observability enhancements, distributed tracing with OpenTelemetry.

- **Fleet Management**: Multi-cluster federation via Rancher or Flux, centralized policy enforcement, unified monitoring aggregation.

- **Dynamic Bitstream Management**: Custom Resource Definitions for bitstream lifecycle, operator pattern for on-demand loading/unloading, partial reconfiguration support.

- **Advanced Scheduling**: Energy-aware workload placement, latency-aware scheduling, predictive scaling, multi-objective optimization.

---

## 8. Conclusions and Future Directions

### 8.1 Summary of Contributions

This report has presented a comprehensive framework for hardware-software co-design and co-creation of neuromorphic AI systems operating in the edge-cloud continuum. The framework addresses the multifaceted challenges of deploying FPGA-accelerated brain-inspired computing through principled architectural design, specialized orchestration mechanisms, and human-centric development methodologies.

The five-layer architecture provides clean separation of concerns from application logic through orchestration, runtime environments, hardware abstraction, and physical infrastructure. This layered design enables independent evolution of components while maintaining well-defined interfaces, facilitating technology substitution and incremental enhancement without systemic disruption.

The FPGA device plugin methodology extends standard Kubernetes orchestration to accommodate specialized hardware resources, implementing discovery, advertisement, allocation, and health monitoring through established extension mechanisms. This approach maintains compatibility with cloud-native tooling and operational practices while addressing FPGA-specific requirements including bitstream management and device-level resource tracking.

The iterative co-design process bridges software algorithm development with hardware implementation through continuous feedback cycles. This methodology enables mutual optimization between algorithmic requirements and hardware capabilities, addressing the tension between computational efficiency and implementation complexity through principled tradeoff exploration.

The security framework implements defense-in-depth across multiple layers from infrastructure through application, recognizing that hardware acceleration introduces elevated privilege requirements that must be carefully managed through isolation mechanisms, least-privilege policies, and comprehensive audit trails.

### 8.2 Lessons Learned and Best Practices

Several key insights emerged from the design and implementation process:

- **Pareto Principle Application**: The explicit decision to defer complex features until core functionality is proven stable significantly reduced implementation risk and enabled early stakeholder feedback. The 80/20 rule proved applicable: fundamental orchestration capabilities deliver majority value, while advanced features provide incremental benefits at disproportionate complexity cost.

- **Standardization Value**: Leveraging mature, standardized platforms (Kubernetes, OCI containers, gRPC APIs) rather than custom solutions dramatically improved system reliability, reduced development effort, and enabled reuse of existing tools and expertise. The device plugin framework specifically provides a stable, well-documented extension point for specialized hardware integration.

- **Co-Creation Importance**: Sustained engagement with stakeholders including end users, developers, and domain experts proved essential for identifying implicit requirements, uncovering edge cases, and ensuring technical solutions align with operational realities and social expectations.

- **Version Synchronization Criticality**: Mismatches between host and container runtime versions (particularly XRT) emerged as a primary source of deployment failures. Rigorous version locking and automated verification in CI/CD pipelines proved necessary for reliable operations.

### 8.3 Future Research Directions

Several research directions warrant future investigation:

- **Automated Co-Design Exploration**: Development of automated design space exploration tools that jointly optimize algorithmic parameters and hardware configurations based on multi-objective cost functions incorporating performance, energy, area, and accuracy metrics.

- **Neuromorphic-Native Orchestration**: Investigation of orchestration primitives specifically tailored to neuromorphic computing characteristics including event-driven processing, temporal dynamics, and non-von-Neumann execution models.

- **Federated Learning Integration**: Adaptation of federated learning algorithms to neuromorphic edge-cloud architectures, addressing challenges of model aggregation, privacy preservation, and communication efficiency for spiking neural networks.

- **Energy-Aware Orchestration**: Development of scheduling policies that explicitly optimize for energy consumption through dynamic workload placement, frequency scaling coordination, and sleep mode management.

- **Explainability for Neuromorphic Systems**: Novel explanation generation techniques appropriate for spiking neural networks and other neuromorphic models, addressing the unique challenges posed by event-driven, temporal computation.

### 8.4 Impact and Societal Considerations

The deployment of neuromorphic AI systems in production environments carries significant societal implications that extend beyond technical implementation concerns. The co-creation framework explicitly incorporates stakeholder perspectives on trustworthiness, transparency, and ethical AI deployment, recognizing that technical excellence alone proves insufficient for responsible AI system development.

Energy efficiency improvements enabled by neuromorphic computing address growing concerns about AI's environmental impact. FPGAs configured for neuromorphic algorithms demonstrate power efficiency approaching specialized ASICs while maintaining post-deployment adaptability. This efficiency enables AI deployment in resource-constrained edge environments where power availability limits computational capabilities.

The accessibility of brain-inspired computing to broader developer communities depends critically on abstraction quality and operational simplicity. The orchestration framework presented herein lowers barriers to neuromorphic system deployment through familiar Kubernetes interfaces, automated resource management, and comprehensive operational tooling. This democratization of advanced computing paradigms accelerates innovation by enabling experimentation without requiring deep hardware expertise.

### 8.5 Concluding Remarks

The transition from research prototypes to production-ready neuromorphic AI systems requires comprehensive solutions addressing not only algorithmic and hardware implementation challenges but also orchestration, deployment, security, and operational management concerns. This report has presented such a holistic framework, grounded in principled co-design methodologies and validated operational practices.

The edge-cloud continuum architecture enables flexible workload placement that adapts to application requirements, infrastructure capabilities, and operational constraints. Kubernetes-based orchestration provides mature, battle-tested mechanisms for distributed system management while device plugins enable seamless integration of specialized accelerators.

The human-centric co-creation approach ensures that technical design decisions reflect stakeholder values, operational realities, and societal expectations. Sustained engagement throughout design, implementation, and deployment phases improves system quality while building trust and acceptance among users and affected communities.

As neuromorphic computing matures from laboratory curiosity to practical technology, frameworks such as that presented in this report will prove essential for bridging the gap between innovative algorithms and deployable systems. The anonymized neuromorphic FPGA project contributes both technical solutions and methodological guidance that advance the state of practice in brain-inspired AI systems, paving the way for broader adoption of these promising computational paradigms.

---

## References

[1] Davies, M., et al. (2021). Advancing neuromorphic computing with Loihi: A survey of results and outlook. Proceedings of the IEEE, 109(5), 911-934.

[2] Kubernetes Device Plugin Framework. https://kubernetes.io/docs/concepts/extend-kubernetes/compute-storage-net/device-plugins/

[3] KubeEdge Documentation. https://kubeedge.io/docs/

[4] Xilinx Runtime (XRT) Documentation. https://xilinx.github.io/XRT/

[5] Internal hardware architecture specification, 2024.

[6] Internal hardware and software co-design technical report, 2024.

[7] National Security Agency. Kubernetes Hardening Guide. NSA Cybersecurity Technical Report, 2022.

[8] K3s Lightweight Kubernetes. https://docs.k3s.io/

[9] WasmEdge Runtime Documentation. https://wasmedge.org/docs/

[10] Jonke, Z., Habenschuss, S., & Maass, W. (2016). Solving constraint satisfaction problems with networks of spiking neurons. Frontiers in Neuroscience, 10, 118.

---

## Appendix A: Deployment Configuration Examples

### A.1 Sample Pod Specification with FPGA Resources

The following YAML configuration demonstrates a complete pod specification requesting FPGA resources with appropriate security context and resource constraints:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: neuromorphic-inference
  labels:
    app: neuromorphic-ai
spec:
  nodeSelector:
    hardware.fpga.local/fpga: "true"
  containers:
  - name: inference
    image: neuromorphic-ai:v1.0.0
    resources:
      limits:
        xilinx.com/fpga-zcu104: 1
        memory: "2Gi"
        cpu: "2"
      requests:
        xilinx.com/fpga-zcu104: 1
        memory: "1Gi"
        cpu: "1"
    securityContext:
      runAsNonRoot: true
      runAsUser: 1000
      capabilities:
        drop: ["ALL"]
      readOnlyRootFilesystem: false
```

### A.2 Device Plugin DaemonSet Configuration

DaemonSet manifest for deploying the FPGA device plugin to appropriate nodes:

```yaml
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: fpga-device-plugin
  namespace: kube-system
spec:
  selector:
    matchLabels:
      app: fpga-device-plugin
  template:
    metadata:
      labels:
        app: fpga-device-plugin
    spec:
      nodeSelector:
        hardware.fpga.local/fpga: "true"
      hostNetwork: true
      containers:
      - name: device-plugin
        image: fpga-device-plugin:v1.0.0
        securityContext:
          privileged: true
        volumeMounts:
        - name: device-plugin
          mountPath: /var/lib/kubelet/device-plugins
        - name: dev
          mountPath: /dev
        - name: sys
          mountPath: /sys
      volumes:
      - name: device-plugin
        hostPath:
          path: /var/lib/kubelet/device-plugins
      - name: dev
        hostPath:
          path: /dev
      - name: sys
        hostPath:
          path: /sys
```

---

**END OF REPORT**
