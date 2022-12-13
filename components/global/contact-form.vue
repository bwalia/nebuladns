<template>
    <form class="contact-form" 
        novalidate="true"
        name="contact-form"
        @submit.prevent="validateForm"
        method="POST"
        action="/thanks-enquiry-sent"> 

        <nuxt-link to="/thanks-enquiry-sent" style="display:none;">.</nuxt-link>

        <template v-for="(field,i) in fields">

            <form-input 
                v-if="(field.name === 'organisation' && organisation) || (field.type === 'text' && field.name !== 'organisation')"
                :class="(field.data ? 'isFilled' : '')"
                :name="field.name"
                type="text"
                :autocomplete="(field.autocomplete ? field.autocomplete : null)"
                :label="field.label"
                :errors="field.errors"
                v-model="field.data" />
            
            <form-input
                v-if="field.name === 'message' && message"
                :class="(field.data ? 'isFilled' : '')"
                name="message"
                type="textarea"
                :label="field.label"
                :errors="field.errors"
                v-model="field.data">      
            </form-input>

            <p v-if="field.type === 'checkbox'" class="u-min">
                <input 
                    type="checkbox" 
                    :id="field.name" 
                    :name="field.name" 
                    :value="field.name"
                    v-model="field.data" />
                <label :for="field.name">
                    {{(statement) 
                        ? statement 
                        : 'I understand that by sending these details that I have understood and agree to Odin Capital Management Terms of business'
                    }}
                </label> 
                <br/>
                <b class="error-message u-error" v-if="field.errors">{{ field.errors }}</b>
            </p>

        </template>

        <input type="submit" :value="(cta) ? cta : 'Send'" />
        
    </form>
</template>

<script>
export default {
    props: ['cta', 'statement', 'message', 'organisation'],
    data() {
        return {
            errors: 0,
            fields: [
                {
                    name: "name",
                    data: null,
                    type: "text",
                    label: "Full name *",
                    autocomplete: "name",
                    errorMessage: "Your name is required",
                    errors: false,
                    error: '',
                    validation: "required"
                },
                {
                    name: "email",
                    data: null,
                    type: "text",
                    label: "Email address *",
                    autocomplete: "email",
                    errorMessage: "A valid email is required",
                    errors: false,
                    error: '',
                    validation: "email"
                },
                {
                    name: "phone",
                    data: null,
                    type: "text",
                    label: "Primary phone number *",
                    autocomplete: "tel",
                    errorMessage: "A phone number is required",
                    errors: false,
                    error: '',
                    validation: "required"
                },
                {
                    name: "organisation",
                    data: null,
                    type: "text",
                    label: "Organisation",
                    autocomplete: "organization"
                },
                {
                    name: "message",
                    data: null,
                    type: "textarea",
                    label: "Message"
                },
                {
                    name: "consent",
                    data: null,
                    type: "checkbox",
                    label: (this.statement)
                        ? this.statement
                        : 'I understand that by sending these details that I have understood and agree to Odin Capital Management Terms of business',
                    errorMessage: "Please provide your consent",
                    errors: false,
                    error: '',
                    validation: "required"
                }
            ]
        }
    },
    methods: {
        processAndSend: function() {
            let formString = this.$config.baseAPIURL+'/sendEmail';
            formString += '?location=' + 'contact';
            this.fields.forEach(function(item, index) {
                if (item.data && item.name && item.data.length > 0) {
                    formString += '&';
                    formString += item.name;
                    formString += '=';
                    formString += encodeURIComponent(item.data);
                }
            });
            console.log(formString);

            try {
                fetch(formString)
                    .then(response => response.text())
                    .then(data => {
                        console.log(data);

                        if (data.indexOf('success') > -1) {
                            this.$router.push({
                                path: '/contact-form-thanks'   
                            })
                        } else {
                            console.log('Something went wrong')
                        }
                    });
            } catch(error) {
                return error;
            }
        },
        validateForm: function(e) {
            this.errors = 0;
            var vm = this;
            var emailRegex = /^(([^<>()[\]\\.,;:\s@"]+(\.[^<>()[\]\\.,;:\s@"]+)*)|(".+"))@((\[[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\])|(([a-zA-Z\-0-9]+\.)+[a-zA-Z]{2,}))$/;

            this.fields.forEach(function(item, index) {
                item.errors = '';
                if ( !item.data && item.validation === 'required') {
                    item.errors = JSON.parse(JSON.stringify(item.errorMessage));
                    vm.errors += 1;                    
                }
                if ( item.validation === 'email' && !emailRegex.test(item.data)) {
                    item.errors = JSON.parse(JSON.stringify(item.errorMessage));
                    vm.errors += 1;
                }
            });
            console.log(this.errors);

            if (this.errors === 0) {
                console.log('send data');
            }
            e.preventDefault();
            this.processAndSend();
        }
    }
}
</script>


<style lang="css" scoped>

@import "~/assets/css/variables.css";

input[type="submit"] {
    background: var(--foil);
    color: #fff;
    padding: 0.75em 2em ;
    font-size: 1em;
    font-weight: bold;
    font-family: var(--font);
    line-height: 1em;
    margin: 1em 0;
    border-radius: 30px;
    border: none;
    -webkit-appearance: none;
}

</style>