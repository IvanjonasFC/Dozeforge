export const appModalStore = $state({
    selectedPackage: null as string | null,
    context: 'general' as 'general' | 'bloatware' | 'battery',
    
    open(pkg: string, context: 'general' | 'bloatware' | 'battery' = 'general') {
        this.selectedPackage = pkg;
        this.context = context;
    },
    
    close() {
        this.selectedPackage = null;
        this.context = 'general';
    }
});
