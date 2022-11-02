
<script>
import {Line} from 'vue-chartjs' 

export default {
    extends: Line,
    props: ['series', 'benchmark', 'labels'],
    data() {
        return {
            options: {
                maintainAspectRatio: false,
                responsive: true
            }
        }
    },
    computed: {
        chartObject: function() {
            return {
                labels: this.labels,
                datasets: [
                    {
                        label: "% increase since inception",
                        borderColor: "#1790E3",
                        borderWidth: 5,
                        fill: false,
                        data: this.series
                    },
                    {
                        label: "Benchmark",
                        borderColor: "#d5d5d5",
                        borderWidth: 5,
                        fill: false,
                        data: this.benchmark
                    }
                ]
            }
        }
    },
    mounted() {
        this.renderChart(this.chartObject, this.options)
    }
}
</script>


