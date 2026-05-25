export const appModalStore = $state({
    selectedPackage: null as string | null,
    
    open(pkg: string) {
        this.selectedPackage = pkg;
    },
    
    close() {
        this.selectedPackage = null;
    }
});
