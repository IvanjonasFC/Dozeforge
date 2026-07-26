export const appModalStore = $state({
    selectedPackage: null as string | null,
    
    open(pkg: string, _source?: string) {
        this.selectedPackage = pkg;
    },
    
    close() {
        this.selectedPackage = null;
    }
});
