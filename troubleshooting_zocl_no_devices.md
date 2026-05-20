# Troubleshooting: ZOCL Loaded pero "0 devices found"

**Problema:** XRT instalado correctamente, ZOCL driver cargado, pero `xrt-smi examine` reporta "0 devices found"
**Sistema:** ZCU104, PetaLinux 2024.2, XRT 2.18.0
**Última Actualización:** Noviembre 20, 2025

---

## Diagnóstico del Problema

Según tu screenshot:

```
ESTADO ACTUAL:
[OK] XRT Version: 2.18.0
[OK] zocl module loaded (lsmod shows 208896 bytes)
[OK] /dev/dri/renderD128 exists with correct permissions
[OK] FPGA manager state: operating
[FAILED] xrt-smi examine: "0 devices found"
[ERROR] dmesg shows: "error -ENXIO: IRQ index 1 not found"
[ERROR] "no of_node; not parsing pinctrl DT"
[ERROR] "zocl_drm axi:zocl_drm@0 on minor 0"
```

**Causa raíz:** ZOCL driver no encuentra el device tree node correcto para el FPGA.

---

## Tabla de Contenidos

1. [Verificaciones Rápidas](#verificaciones-rápidas)
2. [Problema: Device Tree Incorrecto](#problema-device-tree-incorrecto)
3. [Solución 1: Actualizar Device Tree](#solución-1-actualizar-device-tree-recomendado)
4. [Solución 2: Recompilar ZOCL con Debug](#solución-2-debug-zocl-driver)
5. [Solución 3: Usar Platform Devices](#solución-3-platform-devices-alternativos)
6. [Verificación Post-Fix](#verificación-post-fix)

---

## Verificaciones Rápidas

### 1. Verificar Device Tree Actual

```bash
# Ver device tree compilado
dtc -I fs -O dts /proc/device-tree > /tmp/current.dts

# Buscar nodo FPGA
grep -A 20 "zynqmp_fpga" /tmp/current.dts
grep -A 20 "pcap" /tmp/current.dts
grep -A 20 "zocl" /tmp/current.dts

# Verificar FPGA manager
ls -la /sys/class/fpga_manager/
cat /sys/class/fpga_manager/fpga0/name
cat /sys/class/fpga_manager/fpga0/state
```

**Salida esperada:**
```
/sys/class/fpga_manager/fpga0/name: zynqmp-fpga
/sys/class/fpga_manager/fpga0/state: operating
```

### 2. Verificar PCI/Platform Devices

```bash
# ZOCL en Zynq NO usa PCI, usa platform devices
ls -la /sys/bus/platform/devices/ | grep -i fpga
ls -la /sys/bus/platform/devices/ | grep -i zocl
ls -la /sys/bus/platform/devices/ | grep -i zynqmp

# Ver qué devices detecta el kernel
dmesg | grep -i "zynqmp\|fpga\|zocl" | tail -30
```

### 3. Verificar IRQ Assignment

El error "IRQ index 1 not found" indica problema con interrupciones:

```bash
# Ver interrupciones asignadas
cat /proc/interrupts | grep -i fpga
cat /proc/interrupts | grep -i zocl

# Ver device tree IRQ mappings
dtc -I fs /proc/device-tree/amba_pl@0 2>/dev/null | grep -A 5 "interrupt"
```

---

## Problema: Device Tree Incorrecto

### Qué Necesita ZOCL

El driver ZOCL requiere un nodo específico en el device tree:

```dts
/ {
    amba_pl: amba_pl@0 {
        #address-cells = <2>;
        #size-cells = <2>;
        compatible = "simple-bus";
        ranges;

        zyxclmm_drm: zyxclmm_drm {
            compatible = "xlnx,zocl";
            status = "okay";
            interrupt-parent = <&gic>;
            interrupts = <0 89 4>, <0 90 4>, <0 91 4>, <0 92 4>;
        };
    };
};
```

**Problema común:** PetaLinux por defecto NO incluye este nodo si no hay IP blocks en Vivado design.

---

## Solución 1: Actualizar Device Tree (Recomendado)

### Opción A: Modificar System User Device Tree

```bash
# En tu máquina de desarrollo (donde está PetaLinux project)
cd /path/to/petalinux-project

# Editar system-user.dtsi
vim project-spec/meta-user/recipes-bsp/device-tree/files/system-user.dtsi
```

Añadir este contenido:

```dts
/include/ "system-conf.dtsi"
/ {
    /* Reservar memoria para CMA (si no está ya) */
    reserved-memory {
        #address-cells = <2>;
        #size-cells = <2>;
        ranges;

        /* CMA para DMA buffers */
        linux,cma {
            compatible = "shared-dma-pool";
            reusable;
            size = <0x0 0x20000000>; /* 512MB */
            alignment = <0x0 0x2000>;
            linux,cma-default;
        };
    };
};

/* ZOCL device node */
&amba_pl {
    zyxclmm_drm {
        compatible = "xlnx,zocl";
        status = "okay";
        interrupt-parent = <&gic>;
        interrupts = <0 89 4>, <0 90 4>, <0 91 4>, <0 92 4>;

        /* NOTA: memory-region puede causar problemas si CMA ya está reservado arriba
         * Si obtienes errores de "failed to get memory region", ELIMINAR esta línea:
         * memory-region = <&linux_cma>;
         * La memoria CMA ya está disponible globalmente via reserved-memory.
         */
    };
};
```

**Rebuild PetaLinux:**

```bash
# Rebuild device tree
petalinux-build -c device-tree

# Rebuild kernel (para recompilar DTB)
petalinux-build -c kernel

# Package boot files
petalinux-build

# Generar BOOT.BIN
cd images/linux/
petalinux-package --boot \
    --fsbl zynqmp_fsbl.elf \
    --u-boot u-boot.elf \
    --pmufw pmufw.elf \
    --fpga system.bit \
    --force
```

**Copiar a SD card:**

```bash
# Montar SD card (ajustar /dev/sdX según tu sistema)
sudo mount /dev/sdX1 /mnt

# Copiar archivos actualizados
sudo cp BOOT.BIN /mnt/
sudo cp Image /mnt/
sudo cp system.dtb /mnt/

# Desmontar
sudo umount /mnt
```

### Opción B: Modificar Device Tree en Runtime (Testing Rápido)

**ADVERTENCIA:** Esto es temporal, se pierde al reiniciar.

```bash
# En ZCU104 (ya booteado)

# 1. Decompile current DTB
dtc -I dtb -O dts /boot/system.dtb > /tmp/system.dts

# 2. Editar y añadir nodo ZOCL
vi /tmp/system.dts
# Buscar "amba_pl" y añadir el nodo zyxclmm_drm (ver arriba)

# 3. Recompilar DTB
dtc -I dts -O dtb /tmp/system.dts > /tmp/system_fixed.dtb

# 4. Copiar a /boot
cp /tmp/system_fixed.dtb /boot/system.dtb

# 5. Reiniciar
reboot
```

**Verificar después del reboot:**

```bash
# Ver device tree
dtc -I fs /proc/device-tree/amba_pl@0/zyxclmm_drm

# Debe mostrar el nodo
```

---

## Solución 2: Debug ZOCL Driver

Si el device tree está correcto pero sigue sin funcionar:

### Habilitar Debug Logging

```bash
# Descargar ZOCL con debug habilitado
rmmod zocl

# Reload con debug
modprobe zocl dyndbg=+pmf
# O más verboso:
insmod /lib/modules/$(uname -r)/kernel/drivers/gpu/drm/zocl/zocl.ko debug=0xff

# Ver logs
dmesg | tail -50
```

**Buscar en logs:**

```
# Errores comunes:
"no compatible node found"     -> Device tree missing
"failed to get IRQ"             -> IRQ mapping incorrect
"CMA allocation failed"         -> Not enough CMA memory
"ioremap failed"                -> Memory region conflict
"failed to get memory region"   -> Conflicto con memory-region property (ver nota abajo)
```

**IMPORTANTE - Error "failed to get memory region":**

Si ves este error en dmesg:
```
[    X.XXXXX] zocl: failed to get memory region
```

**Causa:** La propiedad `memory-region = <&linux_cma>;` en el nodo ZOCL puede causar conflictos cuando CMA ya está reservado globalmente.

**Solución:** ELIMINAR la línea `memory-region` del device tree:

```dts
&amba_pl {
    zyxclmm_drm {
        compatible = "xlnx,zocl";
        status = "okay";
        /* NO incluir: memory-region = <&linux_cma>; */
    };
};
```

La memoria CMA reservada en `reserved-memory` ya está disponible para todos los drivers que la necesiten.

### Verificar Compatibilidad del Driver

```bash
# Ver qué compatible strings busca ZOCL
modinfo zocl | grep alias

# Salida esperada:
# alias:          of:N*T*Cxlnx,zoclC*
# alias:          of:N*T*Cxlnx,zocl

# Verificar que device tree usa "xlnx,zocl"
grep -r "compatible.*zocl" /proc/device-tree/
```

---

## Solución 3: Platform Devices Alternativos

Si ZOCL no funciona, puedes usar **XOCL** (PCIe-based) en modo emulación:

### Instalar XOCL en vez de ZOCL

```bash
# Descargar ZOCL
rmmod zocl

# Cargar XOCL (si está disponible)
modprobe xocl

# Verificar
lsmod | grep xocl
xrt-smi examine
```

**NOTA:** XOCL está diseñado para FPGAs PCIe (Alveo), no para Zynq embedded. Probablemente **no funcione** en ZCU104.

### Alternativa: Usar XRT en Modo Emulación

```bash
# Set emulation mode
export XCL_EMULATION_MODE=sw_emu

# XRT usará software emulation
xrt-smi examine
# Debería mostrar device virtual
```

**Limitación:** No accederá a FPGA real, solo para testing de software.

---

## Solución 4: Verificar Vivado Design

El problema puede estar en el **hardware design** original.

### Verificar Block Design en Vivado

Si tienes acceso al proyecto Vivado que generó `system.bit`:

```tcl
# En Vivado TCL console
open_project <your_project.xpr>
open_bd_design [get_files *.bd]

# Verificar si hay IP blocks en PL
report_ip_status

# Verificar address map
report_property [get_bd_addr_segs]

# Verificar interrupts
report_property [get_bd_intf_pins -of_objects [get_bd_cells processing_system7_0] -filter {MODE==Master}]
```

**Si PL está vacío** (no hay IP cores):

```tcl
# Añadir dummy AXI IP para forzar amba_pl en device tree
create_bd_cell -type ip -vlnv xilinx.com:ip:axi_gpio:2.0 axi_gpio_0

# Conectar a PS
apply_bd_automation -rule xilinx.com:bd_rule:axi4 \
    -config {Master "/processing_system7_0/M_AXI_GP0" Clk "Auto" } \
    [get_bd_intf_pins axi_gpio_0/S_AXI]

# Regenerate bitstream
reset_run impl_1
launch_runs impl_1 -to_step write_bitstream
wait_on_run impl_1

# Export XSA
write_hw_platform -fixed -force -file system_wrapper.xsa
```

Luego en PetaLinux:

```bash
petalinux-config --get-hw-description=/path/to/system_wrapper.xsa
petalinux-build
```

---

## Solución 5: Usar Minimal ZOCL Device Tree

Si nada funciona, prueba con device tree **mínimo** sin IRQs:

```dts
/* En system-user.dtsi */
&amba {
    zocl_0: zocl@0 {
        compatible = "xlnx,zocl";
        status = "okay";
        /* Sin IRQs - polling mode */
    };
};
```

**Rebuild y test:**

```bash
petalinux-build -c device-tree
# Copiar DTB a ZCU104
# Reboot

# Verificar
xrt-smi examine
```

---

## Verificación Post-Fix

Cuando cualquiera de las soluciones funcione, verificar:

### 1. Device Detectado

```bash
xrt-smi examine

# Salida esperada:
# Device(s) Present
#   1 device found     <- DEBE SER >0
```

### 2. Device Details

```bash
xrt-smi examine -d 0 -r all

# Debe mostrar:
# - Platform: xilinx_zcu104_base
# - Memory: DDR banks
# - Temperature sensors (si disponibles)
```

### 3. Test DMA Buffer

```bash
# Compilar test simple (crear /tmp/test_xrt.cpp)
cat > /tmp/test_xrt.cpp << 'EOF'
#include <xrt/xrt_device.h>
#include <xrt/xrt_bo.h>
#include <iostream>

int main() {
    try {
        auto device = xrt::device(0);
        std::cout << "Device opened successfully\n";

        auto bo = xrt::bo(device, 4096, 0);
        std::cout << "Buffer allocated successfully\n";

        return 0;
    } catch (std::exception& e) {
        std::cerr << "Error: " << e.what() << "\n";
        return 1;
    }
}
EOF

g++ -std=c++17 /tmp/test_xrt.cpp -o /tmp/test_xrt \
    -I/opt/xilinx/xrt/include -L/opt/xilinx/xrt/lib -lxrt_coreutil

/tmp/test_xrt
```

**Salida esperada:**
```
Device opened successfully
Buffer allocated successfully
```

### 4. Ready para WASM Test

```bash
cd /home/root/wasi-fpga-artifact/wasi-fpga
cargo build --release

# Test con WASM module
cd ../workloads/wasm/poc-test
export WASMEDGE_PLUGIN_PATH=/home/root/wasi-fpga-artifact/wasi-fpga/target/release/libwasi_fpga_extensions.so
wasmedge target/wasm32-wasi/release/poc-test.wasm
```

---

## Troubleshooting por Síntoma

### Síntoma: "IRQ index 1 not found"

**Causa:** Device tree no define interrupts correctamente.

**Fix:**
```dts
/* Añadir interrupts al nodo zocl */
interrupts = <0 89 4>, <0 90 4>, <0 91 4>, <0 92 4>;
interrupt-parent = <&gic>;
```

### Síntoma: "no of_node; not parsing pinctrl"

**Causa:** Device tree node no está presente en `/proc/device-tree`.

**Fix:** Verificar device tree (Solución 1).

### Síntoma: "0 devices found" pero ZOCL cargado

**Causa:** ZOCL probe() failed silently.

**Fix:**
```bash
# Ver por qué falló probe
dmesg | grep -i "zocl.*probe"
dmesg | grep -i "zocl.*error"

# Reload con debug
rmmod zocl
modprobe zocl dyndbg=+pmf
dmesg | tail -100
```

### Síntoma: Device tree correcto pero sigue fallando

**Causa:** Mismatch entre XRT version y ZOCL version.

**Fix:**
```bash
# Verificar versions match
xrt-smi examine | grep Version
modinfo zocl | grep version

# Deben ser compatibles (e.g., ambos 2.18.x)
```

---

## Checklist de Debugging

```
1. Hardware
   [ ] FPGA bitstream loaded (system.bit en BOOT.BIN)
   [ ] /sys/class/fpga_manager/fpga0/state = "operating"
   [ ] /dev/dri/renderD128 exists

2. Device Tree
   [ ] amba_pl node exists
   [ ] zocl/zyxclmm_drm node with compatible="xlnx,zocl"
   [ ] IRQ mappings defined
   [ ] CMA memory reserved

3. Driver
   [ ] ZOCL module loaded (lsmod | grep zocl)
   [ ] No errors in dmesg related to zocl
   [ ] /sys/bus/platform/devices/ has zocl entry

4. XRT
   [ ] xrt-smi examine shows >=1 device
   [ ] xrt libraries found (ldd test program)
   [ ] Can allocate DMA buffers
```

---

## Próximos Pasos

Una vez que `xrt-smi examine` muestre "1 device found":

1. **Continuar con Phase 2 testing:**
   - Ver [testing_without_bitstream.md](testing_without_bitstream.md)
   - Ejecutar loopback test

2. **Validar WASI stack:**
   - Build wasi-fpga extensions
   - Test con poc-test.wasm

3. **Documentar configuración final:**
   - Guardar system-user.dtsi working
   - Commit device tree changes

---

## Referencias Útiles

### Device Tree para Zynq UltraScale+
- **DT Binding Docs**: https://xilinx-wiki.atlassian.net/wiki/spaces/A/pages/18841847/Linux+Kernel+Device+Tree
- **ZOCL DT Requirements**: https://xilinx.github.io/XRT/master/html/platforms_partitions.html#zynq-ultrascale-mpsoc

### ZOCL Driver Source
- **GitHub**: https://github.com/Xilinx/XRT/tree/master/src/runtime_src/core/edge/drm/zocl
- **Device Probe**: `zocl_drv.c` - ver función `zocl_probe()`

### PetaLinux Device Tree Customization
- **Guide**: https://docs.amd.com/r/en-US/ug1144-petalinux-tools-reference-guide/Customizing-the-Device-Tree
- **Recipes**: https://docs.amd.com/r/en-US/ug1144-petalinux-tools-reference-guide/device-tree-Recipe

---

**TL;DR para tu caso:**

El problema más probable es **device tree missing ZOCL node**. Solución rápida:

1. Editar `system-user.dtsi` en PetaLinux project
2. Añadir nodo `zyxclmm_drm` con `compatible = "xlnx,zocl"`
3. Rebuild: `petalinux-build -c device-tree && petalinux-build`
4. Copiar nuevo `BOOT.BIN` y `system.dtb` a SD card
5. Reboot ZCU104
6. Verificar: `xrt-smi examine` → debe mostrar "1 device found"
